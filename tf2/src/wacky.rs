use chrono_tz;

use crate::Server;
use tokio_cron_scheduler::{Job, JobBuilder};

pub fn start_job(wacky_server: Server) -> Job {
    JobBuilder::new()
        .with_timezone(chrono_tz::US::Eastern)
        .with_cron_job_type()
        .with_schedule("0 0 18 * * Wed")
        .unwrap()
        .with_run_async(Box::new(move |_uuid, _l| {
			let wacky_server = wacky_server.clone();
            Box::pin(async move {
                println!("Starting wacky wednesday.");
                // enable in cfg
				wacky_server
                    .ftp
                    .add_or_edit_line(
                        "tf/cfg/server.cfg",
                        "// exec wacky_wednesday.cfg",
                        "exec wacky_wednesday.cfg",
                    )
                    .await.unwrap();
				wacky_server
                    .ftp
                    .add_or_edit_line(
                        "tf/cfg/server.cfg",
                        "mapcyclefile \"mapcycle.txt\"",
                        "mapcyclefile \"mapcycle-wacky.txt\"",
                    )
                    .await.unwrap();
				// set player count to 32
				let rs = format!("sm_reserved_slots 0");
				let vmp = format!("sv_visiblemaxplayers 32");
			
				// update player count in server.cfg for persistence
				wacky_server
					.ftp
					.add_or_edit_line("tf/cfg/server.cfg", "sm_reserved_slots", &rs)
					.await.unwrap();
				wacky_server
					.ftp
					.add_or_edit_line("tf/cfg/server.cfg", "sv_visiblemaxplayers", &vmp)
					.await.unwrap();
			
				// immediately activate
				let cmd = format!("exec wacky_wednesday.cfg;{rs};{vmp};sm plugins reload nominations; sm plugins reload nativevotes_mapchooser");
				wacky_server.controller.write().await.run(&cmd).await.unwrap();
				println!("Wacky wednesday enabled.");
			})
        }))
        .build()
        .unwrap()
}

pub fn end_job(wacky_server: Server) -> Job {
    JobBuilder::new()
        .with_timezone(chrono_tz::US::Eastern)
        .with_cron_job_type()
        .with_schedule("0 0 0 * * Thu")
        .unwrap()
        .with_run_async(Box::new(move |_uuid, _l| {
			let wacky_server = wacky_server.clone();
            Box::pin(async move {
                println!("Ending wacky wednesday.");
                // remove wacky line from server.cfg
				wacky_server
                    .ftp
                    .add_or_edit_line(
                        "tf/cfg/server.cfg",
                        "exec wacky_wednesday.cfg",
                        "// exec wacky_wednesday.cfg",
                    )
                    .await
                    .expect("Could not disable wacky wednesday.");
				wacky_server
                    .ftp
                    .add_or_edit_line(
                        "tf/cfg/server.cfg",
                        "mapcyclefile \"mapcycle-wacky.txt\"",
                        "mapcyclefile \"mapcycle.txt\"",
                    )
                    .await.unwrap();
				println!("Wacky wednesday stopped.");
            })
        }))
        .build()
        .unwrap()
}
