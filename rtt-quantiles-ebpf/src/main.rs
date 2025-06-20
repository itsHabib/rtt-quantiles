#![no_std]
#![no_main]

use aya_ebpf::{
    helpers::bpf_probe_read_kernel,
    macros::{fentry, map},
    maps::RingBuf,
    programs::FEntryContext,
};

use aya_log_ebpf::info;

use rtt_quantiles_ebpf::vmlinux::{sock, tcp_sock};

#[map(name = "EVENTS")]
static mut EVENTS: RingBuf = RingBuf::with_byte_size(65536, 0);

#[repr(C)]
pub struct RttEvent {
    pub srtt_us: u32,
    pub src_addr: u32,
    pub dst_addr: u32,
}

#[fentry(function = "tcp_rcv_established")]
pub fn rtt_quantiles(ctx: FEntryContext) -> u32 {
    let (srtt_us, src_addr, dst_addr) = unsafe {
        let sk = ctx.arg::<*const sock>(0);
        let ts = sk as *const tcp_sock;

        let srtt_us = bpf_probe_read_kernel(&(*ts).srtt_us).unwrap_or(0) >> 3;

        let inner = &(*sk).__sk_common.__bindgen_anon_1.__bindgen_anon_1;
        let src_addr = u32::from_be(inner.skc_rcv_saddr);
        let dst_addr = u32::from_be(inner.skc_daddr);

        (srtt_us, src_addr, dst_addr)
    };

    let event = RttEvent {
        srtt_us,
        src_addr,
        dst_addr,
    };

    unsafe {
        let _ = EVENTS.output(&event, 0);
        if let Some(mut slot) = EVENTS.reserve((size_of::<RttEvent>() as u64)) {
            core::ptr::write(
                slot.as_mut_ptr() as *mut RttEvent,
                RttEvent {
                    srtt_us,
                    src_addr,
                    dst_addr,
                },
            );
            slot.submit(0);
        }
    }
    0
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[link_section = "license"]
#[no_mangle]
static LICENSE: [u8; 13] = *b"Dual MIT/GPL\0";
