use rand::{distributions::Alphanumeric, Rng};
use shared_memory::{ShmemConf, ShmemError};
use std::{
    path::Path,
    process::{Command, Stdio},
    sync::mpsc::{channel, Receiver, Sender},
};

const WORKER_FILE: &str = "stablegui_worker.exe";

pub struct WorkerInstance {
    #[allow(dead_code)]
    process: std::process::Child,
    sender: Sender<MemoryMessage>,
}

impl WorkerInstance {
    pub fn new() -> std::io::Result<WorkerInstance> {
        // Set up shared memory
        let (sender, thread_receiver) = channel();
        let (thread_sender, receiver) = channel();

        std::thread::spawn(move || memory_thread(thread_receiver, thread_sender));

        let shmem_flink = match receiver.recv() {
            // errors
            Ok(MemoryResponse::ShmemError(e)) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Shmem error: {e}"),
            )),
            Err(e) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Error receiving response: {e}"),
            )),
            // pog
            Ok(MemoryResponse::Flink(shmem_flink)) => Ok(shmem_flink),
        }?;

        println!("Shmem flink: {shmem_flink}");

        // Make process
        let exe_dir = std::env::current_exe();
        let worker_dir = match &exe_dir {
            Ok(path) => path.parent().unwrap(),
            Err(_) => Path::new("."),
        };
        let worker_path = worker_dir.join(WORKER_FILE);
        println!("Running worker at {worker_path:?}");
        let process = Command::new(worker_path)
            .arg(shmem_flink)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;

        // Pog
        Ok(WorkerInstance { process, sender })
    }

    pub fn add_element(&mut self, element: &stablegui_transfer::Element) {
        let mut data = vec![];
        data.reserve(1024);

        stablegui_transfer::encode_element(&mut data, element);

        self.sender.send(MemoryMessage::Send(data)).unwrap();
    }
}

enum MemoryMessage {
    Send(Vec<u8>),
}

enum MemoryResponse {
    ShmemError(ShmemError),
    Flink(String),
}

fn memory_thread(request: Receiver<MemoryMessage>, response: Sender<MemoryResponse>) {
    // make shared memory
    let shmem_id: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect();
    let shmem_flink = format!("stablegui-{}", shmem_id);
    let mut shmem = match ShmemConf::new().size(4096).flink(&shmem_flink).create() {
        Ok(m) => m,
        Err(e) => {
            response.send(MemoryResponse::ShmemError(e)).unwrap();
            return;
        }
    };

    response
        .send(MemoryResponse::Flink(shmem_flink.clone()))
        .unwrap();

    // event loop
    loop {
        match request.recv() {
            Ok(MemoryMessage::Send(new_data)) => unsafe {
                let slice = shmem.as_slice_mut();

                if new_data.len() > slice.len() {
                    eprintln!("Data too large for shared memory");
                    continue;
                }

                slice.copy_from_slice(&new_data);
            },
            Err(e) => {
                eprintln!("Error receiving message: {e}");
                return;
            }
        }
    }
}
