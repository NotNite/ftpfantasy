use ironworks::{
    sqpack::{Install, SqPack},
    Ironworks,
};
use isahc::ReadResponseExt;
use std::{io::Read, path::PathBuf};
use vfs::{MemoryFS, VfsPath};

mod xivvfs;
use xivvfs::XivVfs;

pub async fn get_paths() -> anyhow::Result<Vec<String>> {
    let mut req = isahc::get("https://rl2.perchbird.dev/download/export/CurrentPathList.gz")?;
    let bytes = req.bytes()?;

    let mut decoder = flate2::read::GzDecoder::new(&bytes[..]);
    let mut s = String::new();
    decoder.read_to_string(&mut s)?;

    Ok(s.lines().map(|s| s.to_string()).collect())
}

pub fn build_vfs(paths: Vec<String>) -> anyhow::Result<VfsPath> {
    let vfs: VfsPath = MemoryFS::new().into();

    let mut i = 0;
    let len = paths.len();

    for path in paths {
        let parts = path.split('/').collect::<Vec<_>>();

        let mut last = vfs.root();
        for dir_name in &parts[..parts.len() - 1] {
            let dir = last.join(dir_name)?;

            if !dir.exists()? {
                dir.create_dir()?;
            }

            last = dir;
        }

        let filename = parts.last().unwrap();
        let file = last.join(filename)?;
        file.create_file()?;

        i += 1;

        if i % 100_000 == 0 {
            let percent = i * 100 / len;
            println!("{}/{} ({}%)", i, len, percent);
        }
    }

    Ok(vfs)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args();
    let game_path = args.nth(1).unwrap();
    let address = args.nth(2).unwrap_or_else(|| "0.0.0.0:21910".to_string());

    println!("game path: {}, bind address: {}", game_path, address);

    println!("fetching file list from rl2...");
    let paths = get_paths().await?;
    println!("building vfs... (this may take a while)");
    let vfs = build_vfs(paths)?;

    let server = libunftp::Server::new(Box::new(move || {
        let mut ironworks = Ironworks::new();
        ironworks.add_resource(SqPack::new(Install::at(&PathBuf::from(game_path.clone()))));

        XivVfs {
            ironworks,
            vfs: vfs.clone(),
        }
    }))
    // hardcode some bitches
    .passive_host("lmaobox.n2.pm")
    .passive_ports(std::ops::Range {
        start: 21000,
        end: 22000,
    });

    println!("ftpfantasy ready to rumble");
    server.listen(address).await?;

    Ok(())
}
