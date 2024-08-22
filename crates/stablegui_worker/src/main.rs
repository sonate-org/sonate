use std::{env, time::Duration};

use shared_memory::ShmemConf;

fn main() {
    println!("worker: Hello, world!");

    let args: Vec<String> = env::args().collect();
    let shmem_flink = args.get(1).expect("worker: No arguments provided");
    println!("worker: flink = {:?}", shmem_flink);
    let shmem = ShmemConf::new()
        .flink(&shmem_flink)
        .open()
        .expect("worker: Unable to open shared memory");

    std::thread::sleep(Duration::from_secs(2));

    let data = unsafe { &shmem.as_slice()[0..3] };

    println!(
        "worker: Message received! {:?}",
        String::from_utf8(data.into())
    );
}
