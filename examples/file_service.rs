use std::path::PathBuf;
use std::time::Duration;

use structopt::StructOpt;

use jbhttp::handler::directory::DirectoryHandler;
use jbhttp::prelude::*;
use jbhttp::server::TcpServer;

#[derive(Debug, StructOpt)]
#[structopt(name = "file_service", about = "Example file server.")]
struct Opt {
    #[structopt(short, long, default_value = "8080")]
    port: u16,
    #[structopt(short, long, parse(from_os_str), default_value = "./")]
    dir: PathBuf,
    #[structopt(long, default_value = "1")]
    threads: usize,
    #[structopt(long, default_value = "10")]
    timeout: u64,
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: usize,
}

fn timeout(seconds: u64) -> Option<Duration> {
    if seconds == 0 {
        None
    } else {
        Some(Duration::from_secs(seconds))
    }
}

fn main() {
    let opt = Opt::from_args();

    stderrlog::new()
        .module(module_path!())
        .module("jbhttp")
        .verbosity(opt.verbose)
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();

    let handler = DirectoryHandler::new(&opt.dir).unwrap();
    let serve_dir = handler.root.clone();
    let mut server = TcpServer::new(
        &format!("0.0.0.0:{}", opt.port),
        opt.threads,
        timeout(opt.timeout),
        handler,
    )
    .unwrap();
    println!(
        "Serving {0}, check out: http://localhost:{1}",
        &serve_dir.to_string_lossy(),
        opt.port
    );
    server.serve_forever();
}
