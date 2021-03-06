


extern crate winapi;
extern crate ws2_32;
extern crate kernel32;
extern crate iocp;
extern crate libc;

use self::winapi::winsock2::*;
use self::winapi::ws2def::*;
use self::winapi::minwinbase::*;

use std::io::prelude::*;
use std::net::TcpStream;

use self::ws2_32::*;


use std::boxed;


use std::os::raw::{c_ulong,c_void};
use std::ptr::null;


use std::thread;
use std::boxed::Box;
use std::slice;

use std::collections::HashMap;

use std::os::windows::io::AsRawSocket;
use std::sync::{Mutex,Arc};

use std::cell::RefCell;
use std::borrow::Borrow;
use std::ops::Deref;


type CallbackStore<T> = Arc<Mutex<HashMap<usize, ( Box<Fn(T) + Send> , usize)  >>>;


pub struct AsyncIO {
    iocp:iocp::IoCompletionPort,
    handlers:CallbackStore<&'static [u8]>,
}

impl AsyncIO {

    
    pub fn new()->Option<Arc<AsyncIO>> {
        match iocp::IoCompletionPort::new(4)  {
            Ok(iocp) => {
                let mut aio_bare =AsyncIO {
                    iocp:iocp,
                    handlers:Arc::new(Mutex::new(HashMap::new())),
                };
                let mut aio = Arc::new(aio_bare);
                AsyncIO::init(aio.clone());
                return Some(aio);
            },
            Err(err) => {
                print!("Failed to start iocp");
                return None;
            } 
        }
    }
    
    pub fn register(&self,tcp_stream:&TcpStream){
        unsafe { self.iocp.associate(tcp_stream.as_raw_socket() as *mut c_void,0) };
    }

    
    
    fn init(aio:Arc<AsyncIO>){
    

        thread::spawn(move||{
            println!("aio thread started");
            loop {
            
                match aio.iocp.get_queued(600000) {
                   
                    Ok(status) => {
                        println!("Result {}",status.overlapped as usize);
                        let overlapped_ptr = status.overlapped as usize;
                        let handlers = aio.handlers.try_lock().unwrap();
                        let handler = handlers.get(&overlapped_ptr).unwrap();
                        
                        let wsa_buf_boxed = unsafe { Box::<WSABUF>::from_raw(handler.1 as *mut winapi::ws2def::WSABUF) };
                        let mut wsa_buf = *wsa_buf_boxed;
                        
                        // just to free it
                        let overlapped = unsafe { Box::<OVERLAPPED>::from_raw(status.overlapped) };
                        //wsa_buf.len 
                        // this will probably cause memory leak since Im not getting all the buffer but only the filled part
                        let mut buf= unsafe { slice::from_raw_parts(wsa_buf.buf as *mut u8, status.byte_count as usize) };
                        // will take care of the unused buffer space
                        let unused_buf_ptr = (wsa_buf.buf as usize)+status.byte_count;
                        let unused_buf_size = (wsa_buf.len as usize) - status.byte_count;
                        let unused_buffer_chunk = unsafe { slice::from_raw_parts(unused_buf_ptr as *mut u8,unused_buf_size as usize) };
                        
                        handler.0( buf);
                    
                    },
                    Err(err) => {
                    
                        println!("get_queued error {}",err);
                    
                    }

                }
                
            }
        
        });
    
    
    }



}


pub trait AsyncRead  {
    fn async_read<F: Fn(&[u8]) + Send +'static >(&self,aio:&AsyncIO,handler:F);
}

impl AsyncRead  for TcpStream {

    fn async_read<F: Fn(&[u8]) + Send +'static >(&self,aio:&AsyncIO,handler:F){
        
        const BUFFER_SIZE:usize = 4096;
    
        let mut buf = [0;BUFFER_SIZE];
        let wsa_buf:Box<WSABUF> = Box::new( WSABUF{len:BUFFER_SIZE as u32,buf:buf.as_mut_ptr() as *mut i8 } );
        let lp_wsa_buf: *mut WSABUF = unsafe { Box::into_raw(wsa_buf) };
        
        let dw_buf_count = 1;
        let bytes_len: *mut u32 = unsafe { Box::into_raw(Box::new(0)) };
        
        let mut flags = 0;
        let lp_flags:*mut u32 = &mut flags;
        
        let overlapped = Box::new(OVERLAPPED {
            Internal: 0,
            InternalHigh: 0,
            Offset: 0,
            OffsetHigh: 0,
            hEvent: 0 as *mut c_void,
        });
        let lp_overlapped: *mut OVERLAPPED = unsafe { Box::into_raw(overlapped) };
        
        let buf_ptr = lp_wsa_buf as usize;
        
        let null : *mut libc::c_void;
        let err;
        unsafe {
            err = ws2_32::WSARecv(self.as_raw_socket(),lp_wsa_buf, dw_buf_count, bytes_len ,lp_flags, lp_overlapped, None);
        }
        aio.handlers.try_lock().unwrap().insert(lp_overlapped as usize,(Box::new(handler),buf_ptr));
    }


}




