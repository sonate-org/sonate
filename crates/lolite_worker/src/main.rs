use std::{env, time::Duration};

use shared_memory::ShmemConf;

fn main() {
    println!("worker: Servus!");

    let args: Vec<String> = env::args().collect();
    let shmem_osid = args.get(1).expect("worker: No arguments provided");
    println!("worker: osid = {:?}", shmem_osid);
    let shmem = ShmemConf::new()
        .os_id(shmem_osid)
        .open()
        .expect("worker: Unable to open shared memory");

    std::thread::sleep(Duration::from_secs(2));

    let data = unsafe { &shmem.as_slice()[0..16] };

    println!("worker: Message received! {:?}", &data);
}
