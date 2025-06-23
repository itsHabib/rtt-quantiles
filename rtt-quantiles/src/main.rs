use anyhow::Context as _;
use aya::{programs::FEntry, Btf};
#[rustfmt::skip]
use log::{debug, warn};
use std::{
    collections::VecDeque,
    net::Ipv4Addr,
    ptr,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use aya::maps::RingBuf;
use rtt_tdigest::{Service, Summary};
use tokio::{signal, time};
use aws_config::meta::region::RegionProviderChain;
use aws_config;
use aws_sdk_dynamodb::{Client, Error};
use std::sync::{Arc, Mutex};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RttEvent {
    pub srtt_us: u32,
    pub src_addr: u32,
    pub dst_addr: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    // Bump the memlock rlimit. This is needed for older kernels that don't use the
    // new memcg based accounting, see https://lwn.net/Articles/837122/
    let rlim = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };
    let ret = unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim) };
    if ret != 0 {
        debug!("remove limit on locked memory failed, ret is: {ret}");
    }

    // This will include your eBPF object file as raw bytes at compile-time and load it at
    // runtime. This approach is recommended for most real-world use cases. If you would
    // like to specify the eBPF program at runtime rather than at compile-time, you can
    // reach for `Bpf::load_file` instead.
    let mut ebpf = aya::Ebpf::load(aya::include_bytes_aligned!(concat!(
        env!("OUT_DIR"),
        "/rtt-quantiles"
    )))?;
    if let Err(e) = aya_log::EbpfLogger::init(&mut ebpf) {
        // This can happen if you remove all log statements from your eBPF program.
        warn!("failed to initialize eBPF logger: {e}");
    }
    let btf = Btf::from_sys_fs().context("BTF from sysfs")?;
    let program: &mut FEntry = ebpf.program_mut("rtt_quantiles").unwrap().try_into()?;
    program.load("tcp_rcv_established", &btf)?;
    program.attach()?;

    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let config = aws_config::from_env().region(region_provider).load().await;
    let client =  Client::new(&config);
    let svc = Service::new(
        client,
        "sample-app".to_string(),
        "local".to_string(),
    );

    let events_map = ebpf
        .map_mut("EVENTS")
        .ok_or(anyhow!("EVENTS map not found"))?;
    let mut ringbuf = RingBuf::try_from(events_map)?;
    let start = Instant::now();
    let mut summary_mutex = Arc::new(Mutex::new(Summary::new()));


    tokio::spawn(async move {
        let mut store_interval = time::interval(Duration::from_secs(60));
        let mut summary_mutex = Arc::clone(&summary_mutex);

        loop {
            store_interval.tick().await;
            let digest = match summary_mutex.lock() {
                Ok(summary) => summary.digest(),
                Err(e) => {
                    warn!("Failed to lock summary mutex: {}", e);
                    continue;
                }
            };

            match svc.store_tdigest("1m".to_string(), digest).await {
                Ok(_) => println!("Stored 1m T-Digest successfully"),
                Err(e) => warn!("Failed to store p99 digest: {}", e),
            }
        }
    });


    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                println!("Exiting...");
                break;
            }
            _ = tokio::task::yield_now() => {
                if let Some(data) = ringbuf.next() {
                    let event = unsafe { ptr::read(data.as_ptr() as *const RttEvent) };
                    let mut rtt_summary = match summary_mutex.lock() {
                        Ok(summary) => summary,
                        Err(e) => {
                            warn!("Failed to lock summary mutex: {}", e);
                            continue;
                        }
                    };

                    rtt_summary.add_rtt(event.srtt_us);

                    if rtt_summary.count() % 100 == 0 {
                        let elapsed = start.elapsed().as_secs_f64();
                        let rate = rtt_summary.count() as f64 / elapsed;
                        println!(
                            "📊 {} samples in {:.1}s = {:.1} events/sec",
                            rtt_summary.count(),
                            elapsed,
                            rate
                        );
                        println!(
                            "RTT={}µs src={} dst={}, p99:{:.1}ms, p90:{:.1}ms",
                            event.srtt_us,
                            u32_to_ip(event.src_addr),
                            u32_to_ip(event.dst_addr),
                            rtt_summary.p99(),
                            rtt_summary.p90(),
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

fn u32_to_ip(ip: u32) -> String {
    Ipv4Addr::from(ip).to_string()
}
