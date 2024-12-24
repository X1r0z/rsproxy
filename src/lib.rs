use std::io::Result;

use clap::{Parser, Subcommand};
use forward::Forward;
use proxy::Proxy;
use reuse::Reuse;
use tracing::info;

pub mod crypto;
pub mod forward;
pub mod proxy;
pub mod reuse;
pub mod socks;
pub mod tcp;
pub mod udp;
pub mod util;

#[derive(Parser)]
#[command(author, version, about = "Pivot: Port-Forwarding and Proxy Tool")]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Port forwarding mode
    Fwd {
        /// Local listen IP address, format: [+][IP:]PORT
        #[arg(short, long)]
        local: Vec<String>,

        /// Remote connect IP address, format: [+]IP:PORT
        #[arg(short, long)]
        remote: Vec<String>,

        /// Unix domain socket path
        #[cfg(target_family = "unix")]
        #[arg(short, long)]
        socket: Option<String>,

        /// Enable UDP forward mode
        #[arg(short, long)]
        udp: bool,
    },

    /// Socks proxy mode
    Proxy {
        /// Local listen IP address, format: [+][IP:]PORT
        #[arg(short, long)]
        local: Vec<String>,

        /// Reverse server IP address, format: [+]IP:PORT
        #[arg(short, long)]
        remote: Option<String>,

        /// Authentication info, format: user:pass (other for random)
        #[arg(short, long)]
        auth: Option<String>,
    },

    /// Port reuse mode
    Reuse {
        /// Local reuse IP address, format: IP:PORT
        #[arg(short, long)]
        local: String,

        /// Remote redirect IP address, format: IP:PORT
        #[arg(short, long)]
        remote: String,

        /// Fallback IP address, format: IP:PORT
        #[arg(short, long)]
        fallback: Option<String>,  // Make fallback optional

        /// External IP address, format: IP
        #[arg(short, long)]
        external: String,
    },
}

pub async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Fwd {
            local,
            remote,
            #[cfg(target_family = "unix")]
            socket,
            udp,
        } => {
            info!("Starting forward mode");

            if udp {
                info!("Using UDP protocol");
            } else {
                info!("Using TCP protocol");
            }

            let local_addrs = local
                .iter()
                .map(|addr| addr.replace("+", ""))
                .map(|addr| match addr.contains(':') {
                    true => addr,
                    false => format!("0.0.0.0:{}", addr),
                })
                .collect();
            let remote_addrs = remote.iter().map(|addr| addr.replace("+", "")).collect();

            let local_opts = local.iter().map(|addr| addr.starts_with('+')).collect();
            let remote_opts = remote.iter().map(|addr| addr.starts_with('+')).collect();

            let forward = Forward::new(
                local_addrs,
                remote_addrs,
                local_opts,
                remote_opts,
                #[cfg(target_family = "unix")]
                socket,
                udp,
            );

            forward.start().await?;
        }
        Commands::Proxy {
            local,
            remote,
            auth,
        } => {
            info!("Starting proxy mode");

            let local_addrs = local
                .iter()
                .map(|addr| addr.replace("+", ""))
                .map(|addr| match addr.contains(':') {
                    true => addr,
                    false => format!("0.0.0.0:{}", addr),
                })
                .collect();
            let local_opts = local.iter().map(|addr| addr.starts_with('+')).collect();

            let remote_addr = remote.as_ref().map(|addr| addr.replace("+", ""));
            let remote_opt = remote.is_some_and(|addr| addr.starts_with('+'));

            let auth_info = auth.map(|v| socks::AuthInfo::new(v));

            let proxy = Proxy::new(local_addrs, remote_addr, local_opts, remote_opt, auth_info);
            proxy.start().await?;
        }
        Commands::Reuse {
            local,
            remote,
            fallback,
            external,
        } => {
            info!("Starting reuse mode");

            let reuse = Reuse::new(local, remote, fallback, external);
            reuse.start().await?;
        }
    }

    Ok(())
}