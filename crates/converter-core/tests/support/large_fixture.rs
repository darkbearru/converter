use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::Path,
};

pub struct FixtureSize {
    pub articles: u64,
    pub bytes: u64,
}

pub fn generate(path: &Path, articles: u64) -> io::Result<FixtureSize> {
    let mut out = BufWriter::new(File::options().write(true).create_new(true).open(path)?);
    writeln!(out, "#NAME \"Синтетический крупный словарь\"")?;
    for index in (0..articles).rev() {
        writeln!(
            out,
            "word{index:08}\nalias{index:08}\nsame\n [b]Значение {index}[/b] lorem ipsum dolor sit amet [ref]word00000000[/ref]"
        )?;
    }
    out.flush()?;
    Ok(FixtureSize {
        articles,
        bytes: out.get_ref().metadata()?.len(),
    })
}
