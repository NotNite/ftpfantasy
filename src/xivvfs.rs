use async_trait::async_trait;
use ironworks::Ironworks;
use libunftp::auth::DefaultUser;
use libunftp::storage::{ErrorKind, Fileinfo, Metadata, Result, StorageBackend};
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use vfs::VfsPath;

#[derive(Debug)]
pub struct XivVfs {
    pub ironworks: Ironworks,
    pub vfs: VfsPath,
}

#[derive(Debug)]
pub struct Meta {
    file_size: usize,
    is_dir: bool,
    is_file: bool,
}

impl XivVfs {
    fn get_metadata(&self, path: String) -> anyhow::Result<Meta> {
        //println!("(get_metadata)path: {}", path);

        let file_size = self.ironworks.file::<Vec<u8>>(&path).map(|x| x.len());
        //println!("(get_metadata)file_size: {:?}", file_size);

        let file = self.vfs.root().join(&path)?;

        Ok(Meta {
            file_size: file_size.unwrap_or(0),
            is_dir: file.is_dir()?,
            is_file: file.is_file()?,
        })
    }

    fn get_full_path(&self, path: VfsPath) -> String {
        let mut deez = Vec::new();

        deez.push(path.filename());

        let mut parent = path.parent();
        while let Some(parent2) = parent {
            deez.push(parent2.filename());
            parent = parent2.parent();
        }

        deez.reverse();
        //println!("(get_full_path)deez: {:?}", deez);

        let mut str = deez.join("/");
        if str.starts_with('/') {
            str = str[1..].to_string();
        }

        //println!("(get_full_path)str: {}", str);

        if str.is_empty() {
            "/".to_string()
        } else {
            str
        }
    }

    fn fix_path<P: AsRef<Path>>(&self, path: P) -> String {
        let mut path = path
            .as_ref()
            .to_str()
            .unwrap()
            .to_string()
            .replace('\\', "/"); // WINDOWS MOMENT!!!!!

        if path.starts_with('/') && path.len() > 1 {
            path = path[1..].to_string();
        }

        let mut path_vfs = self.vfs.root();
        if !path.is_empty() && path != "/" {
            path_vfs = path_vfs.join(path).unwrap();
        }
        //println!("(fix_path)path_vfs: {:?}", path_vfs);

        self.get_full_path(path_vfs)
    }
}

#[async_trait]
impl libunftp::storage::StorageBackend<DefaultUser> for XivVfs {
    type Metadata = Meta;

    async fn metadata<P: AsRef<Path> + Send + Debug>(
        &self,
        _: &DefaultUser,
        path: P,
    ) -> Result<Self::Metadata> {
        let metadata = self.get_metadata(self.fix_path(path));
        //println!("(metadata)metadata: {:?}", metadata);

        metadata.map_err(|_| ErrorKind::LocalError.into())
    }

    async fn list<P: AsRef<Path> + Send + Debug>(
        &self,
        _: &DefaultUser,
        path: P,
    ) -> Result<Vec<Fileinfo<PathBuf, Self::Metadata>>>
    where
        <Self as StorageBackend<DefaultUser>>::Metadata: Metadata,
    {
        let path = self.fix_path(path);
        //println!("(list)path: {}", path);

        let mut dir = self.vfs.root();
        if path != "/" {
            dir = dir.join(path).map_err(|_| ErrorKind::LocalError)?;
        }

        let mut files = Vec::new();
        for file in dir.read_dir().map_err(|_| ErrorKind::LocalError)? {
            //println!("(list)file: {:?}", file);
            let full_path = self.get_full_path(file);
            //println!("(list)full_path: {}", full_path);

            let metadata = self.get_metadata(full_path.clone());
            //println!("(list)metadata: {:?}", metadata);

            let metadata_two = metadata.map_err(|_| ErrorKind::LocalError)?;

            files.push(Fileinfo {
                path: PathBuf::from(full_path),
                metadata: metadata_two,
            });
        }

        Ok(files)
    }

    async fn get<P: AsRef<Path> + Send + Debug>(
        &self,
        _: &DefaultUser,
        path: P,
        start_pos: u64,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Sync + Unpin>> {
        let path = self.fix_path(path);
        //println!("(get)path: {}", path);

        let file = self.ironworks.file::<Vec<u8>>(&path);
        if let Ok(mut file) = file {
            file.drain(..start_pos as usize);
            Ok(Box::new(std::io::Cursor::new(file)))
        } else {
            Err(ErrorKind::PermanentFileNotAvailable.into())
        }
    }

    async fn cwd<P: AsRef<Path> + Send + Debug>(&self, _: &DefaultUser, path: P) -> Result<()> {
        let path = self.fix_path(path);
        //println!("(cwd)path: {}", path);

        let mut dir = self.vfs.root();

        if path != "/" {
            dir = dir.join(path).map_err(|_| ErrorKind::LocalError)?;
        }

        if dir.exists().map_err(|_| ErrorKind::LocalError)? {
            Ok(())
        } else {
            Err(ErrorKind::PermanentFileNotAvailable.into())
        }
    }

    async fn put<
        P: AsRef<Path> + Send + Debug,
        R: tokio::io::AsyncRead + Send + Sync + Unpin + 'static,
    >(
        &self,
        _: &DefaultUser,
        _: R,
        _: P,
        _: u64,
    ) -> Result<u64> {
        Err(ErrorKind::CommandNotImplemented.into())
    }

    async fn del<P: AsRef<Path> + Send + Debug>(&self, _: &DefaultUser, _: P) -> Result<()> {
        Err(ErrorKind::CommandNotImplemented.into())
    }

    async fn mkd<P: AsRef<Path> + Send + Debug>(&self, _: &DefaultUser, _: P) -> Result<()> {
        Err(ErrorKind::CommandNotImplemented.into())
    }

    async fn rename<P: AsRef<Path> + Send + Debug>(
        &self,
        _: &DefaultUser,
        _: P,
        _: P,
    ) -> Result<()> {
        Err(ErrorKind::CommandNotImplemented.into())
    }

    async fn rmd<P: AsRef<Path> + Send + Debug>(&self, _: &DefaultUser, _: P) -> Result<()> {
        Err(ErrorKind::CommandNotImplemented.into())
    }
}

impl Metadata for Meta {
    fn len(&self) -> u64 {
        self.file_size as u64
    }

    fn is_dir(&self) -> bool {
        self.is_dir
    }

    fn is_file(&self) -> bool {
        self.is_file
    }

    fn is_symlink(&self) -> bool {
        false
    }

    fn modified(&self) -> Result<SystemTime> {
        Ok(SystemTime::UNIX_EPOCH)
    }

    fn gid(&self) -> u32 {
        0
    }

    fn uid(&self) -> u32 {
        0
    }
}
