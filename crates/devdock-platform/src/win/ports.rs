//! Listening TCP sockets with their owning PID, via IP Helper's
//! `GetExtendedTcpTable` (T5.1). IPv4 and IPv6 are separate tables.
#![allow(unsafe_code)]

use std::io;
use std::mem::size_of;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use devdock_core::ports::Listener;
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

pub fn tcp_listeners() -> io::Result<Vec<Listener>> {
    let mut out: Vec<Listener> = rows::<MIB_TCPROW_OWNER_PID>(AF_INET.0)?
        .iter()
        .map(|r| Listener {
            port: port_of(r.dwLocalPort),
            pid: r.dwOwningPid,
            // dwLocalAddr holds the address bytes in network order.
            address: IpAddr::V4(Ipv4Addr::from(r.dwLocalAddr.to_ne_bytes())),
        })
        .collect();
    out.extend(
        rows::<MIB_TCP6ROW_OWNER_PID>(AF_INET6.0)?
            .iter()
            .map(|r| Listener {
                port: port_of(r.dwLocalPort),
                pid: r.dwOwningPid,
                address: IpAddr::V6(Ipv6Addr::from(r.ucLocalAddr)),
            }),
    );
    Ok(out)
}

/// The port is in the low 16 bits, in network byte order.
fn port_of(raw: u32) -> u16 {
    u16::from_be((raw & 0xFFFF) as u16)
}

/// Reads one table: `u32 count` followed by `count` rows of type `R`.
/// `R` must be the row type that matches `family` for TCP_TABLE_OWNER_PID_LISTENER.
fn rows<R: Copy>(family: u16) -> io::Result<Vec<R>> {
    let mut size: u32 = 0;
    // Grow until the table fits: it can change size between calls.
    for _ in 0..8 {
        // u64 storage keeps the buffer aligned for the u32 fields inside rows.
        let mut buf: Vec<u64> = vec![0; (size as usize).div_ceil(8).max(1)];
        let mut len = (buf.len() * 8) as u32;
        // SAFETY: `buf` is writable for `len` bytes and outlives the call; on
        // ERROR_INSUFFICIENT_BUFFER Windows writes only the required size to `len`.
        let rc = unsafe {
            GetExtendedTcpTable(
                Some(buf.as_mut_ptr().cast()),
                &raw mut len,
                false,
                u32::from(family),
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if rc == ERROR_INSUFFICIENT_BUFFER.0 {
            size = len;
            continue;
        }
        if rc != NO_ERROR.0 {
            return Err(io::Error::from_raw_os_error(rc as i32));
        }

        let bytes = buf.len() * 8;
        let base = buf.as_ptr().cast::<u8>();
        // SAFETY: the buffer starts with the u32 entry count, written by Windows.
        let count = unsafe { base.cast::<u32>().read() } as usize;
        // Rows start right after the count, aligned for `R` (whose fields are u32/u8).
        let offset = size_of::<u32>().next_multiple_of(align_of::<R>());
        if offset + count * size_of::<R>() > bytes {
            return Err(io::Error::other("TCP table larger than its buffer"));
        }
        // SAFETY: bounds were checked just above; `R` is plain-old-data (Copy, only
        // integers/byte arrays) as filled in by Windows; we copy the rows out.
        let rows = unsafe { std::slice::from_raw_parts(base.add(offset).cast::<R>(), count) };
        return Ok(rows.to_vec());
    }
    Err(io::Error::other("TCP table kept changing size"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn sees_our_own_listener_with_our_pid() {
        let l4 = TcpListener::bind("127.0.0.1:0").unwrap();
        let p4 = l4.local_addr().unwrap().port();
        let l6 = TcpListener::bind("[::1]:0").ok();
        let all = tcp_listeners().unwrap();
        let me = std::process::id();
        assert!(
            all.iter().any(|l| l.port == p4
                && l.pid == me
                && l.address == IpAddr::V4(Ipv4Addr::LOCALHOST)),
            "port {p4} not found"
        );
        if let Some(l6) = l6 {
            let p6 = l6.local_addr().unwrap().port();
            assert!(
                all.iter()
                    .any(|l| l.port == p6 && l.pid == me && l.address.is_ipv6())
            );
        }
    }

    #[test]
    fn port_bytes_are_network_order() {
        // 3000 = 0x0BB8; in network order the low 16 bits read 0xB80B on little endian.
        assert_eq!(port_of(u32::from(3000u16.to_be())), 3000);
    }
}
