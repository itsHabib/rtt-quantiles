#![no_std]
#![no_main]

use aya_ebpf::{
    macros::{fentry, map},
    programs::FEntryContext,
    helpers::bpf_probe_read_kernel,
    cty::{c_void, c_uint},
    bindings::{tcp_sock},
    maps::{RingBuf},
};

use aya_log_ebpf::info;

#[map(name = "EVENTS")]
static mut EVENTS: RingBuf = RingBuf::with_byte_size(65536, 0);




#[repr(C)]
pub struct sockCommon {
    pub skc_daddr: u32,
    pub skc_rcv_saddr: u32,
    // You can add more fields if needed (order must match kernel)
}

#[repr(C)]
pub struct sock {
    pub __sk_common: sockCommon,
    // Padding to match layout, if necessary
}

#[repr(C)]
pub struct TcpSock {
    pub srtt_us: u32,
    // ...
}

#[repr(C)]
pub struct RttEvent {
    pub srtt_us: u32,
    pub src_addr: u32,
    pub dst_addr: u32,
}

#[fentry(function = "tcp_rcv_established")]
pub fn rtt_quantiles(ctx: FEntryContext) -> u32 {
    match try_rtt_quantiles(ctx) {
        Ok(ret) => ret,
        Err(ret) => ret,
    }
}

fn try_rtt_quantiles(ctx: FEntryContext) -> Result<u32, u32> {
    // Get the socket pointer (first argument) - this needs unsafe
    let sk = unsafe { ctx.arg::<*const sock>(0) };
    let ts = sk as *const TcpSock;

    let src_addr = unsafe { (*sk).__sk_common.skc_rcv_saddr };
    let dst_addr = unsafe { (*sk).__sk_common.skc_daddr };
    let srtt_us = unsafe { (*ts).srtt_us  } >> 3;

    let event = RttEvent {
        srtt_us,
        src_addr,
        dst_addr,
    };

    // Write to the ring buffer
    unsafe {
        let _ = unsafe { EVENTS.output(&event, 0) };
    }


   // info!(&ctx, "RTT: {}µs src={} dst={}", srtt_us, src_addr, dst_addr);

    Ok(0)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[link_section = "license"]
#[no_mangle]
static LICENSE: [u8; 13] = *b"Dual MIT/GPL\0";
