use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::sync::oneshot;
use tracing::{info, instrument, Span};

use crate::engine::JsWorker;
use crate::{Req, Res};

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    /// Initialize and run worker in a background thread, get request via mpsc channel
    /// once the request is processed, the response will send back
    /// through an oneshot channel
    fn new(id: usize, code: &str, receiver: Arc<Mutex<Receiver<Message>>>) -> Worker {
        let code = code.to_string();
        let thread = thread::spawn(move || {
            let js = JsWorker::try_new(&code).unwrap();
            loop {
                let message = receiver.lock().unwrap().recv().unwrap();
                match message {
                    Message::NewRequest(req) => {
                        let _span = req.span.enter();

                        info!("Worker {} got a job; executing.", id);
                        let res = js.run(&req.handler, req.req).unwrap();
                        req.tx.send(res).expect("should send");
                    }
                    Message::Terminate => {
                        info!("Worker {} was told to terminate.", id);
                        break;
                    }
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}

pub struct Request {
    req: Req,
    handler: String,
    tx: oneshot::Sender<Res>,
    span: Span,
}

impl Request {
    pub fn new(req: Req, handler: &str, tx: oneshot::Sender<Res>, span: Span) -> Self {
        Self {
            req,
            handler: handler.to_string(),
            tx,
            span,
        }
    }
}

enum Message {
    NewRequest(Request),
    Terminate,
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Sender<Message>,
}

impl ThreadPool {
    /// Initialize thread pool
    ///
    /// `size` is the background threads count
    pub fn new(size: usize, code: &str) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, code, Arc::clone(&receiver)));
        }

        ThreadPool { workers, sender }
    }

    /// Execute task asynchronously
    ///
    /// Return `oneshot::Receiver` for receiving execution result
    /// Caller decides whether to `blocking_recv` or `await` the return value
    #[instrument(skip(self))]
    pub fn execute(&self, handler: &str, req: Req) -> oneshot::Receiver<Res> {
        let (tx, rx) = oneshot::channel();

        let request = Request::new(req, handler, tx, tracing::Span::current());
        self.sender.send(Message::NewRequest(request)).unwrap();
        rx
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        info!("Sending terminate message to all workers.");

        for _ in &self.workers {
            self.sender.send(Message::Terminate).unwrap();
        }

        info!("Shutting down all workers.");

        for worker in &mut self.workers {
            info!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

#[test]
fn thread_pool_works() {
    let code = r#"
    (function(){
        async function hello(req){
            return {
                status:200,
                headers:{
                    "content-type":"application/json"
                },
                body: JSON.stringify(req),
            };
        }
        return{hello:hello};
    })();
    "#;

    let pool = ThreadPool::new(4, code);

    let rx = pool.execute(
        "hello",
        Req::builder()
            .method("GET".to_string())
            .url("/api/hello".to_string())
            .build(),
    );

    let result = rx.blocking_recv();
    println!("The result is: {:?}", result.unwrap());
}
