#![no_std]
#![no_main]

use aya_ebpf::{macros::fentry, programs::FEntryContext};
use aya_log_ebpf::info;

#[fentry(function = "tcp_rcv_established")]
pub fn rtt_quantiles(ctx: FEntryContext) -> u32 {
    match try_rtt_quantiles(ctx) {
        Ok(ret) => ret,
        Err(ret) => ret,
    }
}

fn try_rtt_quantiles(ctx: FEntryContext) -> Result<u32, u32> {
    info!(&ctx, "function tcp_rcv_established called");
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
