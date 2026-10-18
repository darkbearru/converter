use flate2::{Compression, write::ZlibEncoder};
use std::{fs::File, io::Write, path::PathBuf};
use zip::{ZipWriter, write::SimpleFileOptions};

const DSL: &str = "#NAME \"Проверка Converter\"\n#INDEX_LANGUAGE \"Russian\"\n#CONTENTS_LANGUAGE \"English\"\nalpha\nальфа\n [m1][b]~[/b] [p]n.[/p][/m]\n [c red]Красный текст[/c], [i]курсив[/i], [ex]пример перевода[/ex].\n [s]media/picture.png[/s]\n [s]media/tone.wav[/s]\n [ref]beta[/ref]\n @sub{¹}(s)\n  Дочерняя статья со ссылкой [ref]alpha[/ref].\n @\nbeta\nбета\n [b]Вторая статья[/b]. <<alpha>>\n [s]AUX.png[/s]\n [unknown]Неизвестный тег виден[/unknown]\n";

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

fn png_chunk(out: &mut Vec<u8>, name: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(name);
    out.extend_from_slice(body);
    let mut checked = name.to_vec();
    checked.extend_from_slice(body);
    out.extend_from_slice(&crc32(&checked).to_be_bytes());
}

fn picture() -> std::io::Result<Vec<u8>> {
    let mut image = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = 192u32.to_be_bytes().to_vec();
    header.extend_from_slice(&96u32.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    png_chunk(&mut image, b"IHDR", &header);
    let mut compressor = ZlibEncoder::new(Vec::new(), Compression::default());
    for y in 0..96 {
        compressor.write_all(&[0])?;
        for x in 0..192 {
            compressor.write_all(if (x / 24 + y / 24) % 2 == 0 {
                &[32, 94, 190]
            } else {
                &[250, 194, 63]
            })?;
        }
    }
    png_chunk(&mut image, b"IDAT", &compressor.finish()?);
    png_chunk(&mut image, b"IEND", &[]);
    Ok(image)
}

fn tone() -> Vec<u8> {
    let samples = 5512u32;
    let length = samples * 2;
    let mut wav = b"RIFF".to_vec();
    wav.extend_from_slice(&(36 + length).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&22050u32.to_le_bytes());
    wav.extend_from_slice(&44100u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&length.to_le_bytes());
    for sample in 0..samples {
        let value =
            ((f64::from(sample) * 440.0 * std::f64::consts::TAU / 22050.0).sin() * 4000.0) as i16;
        wav.extend_from_slice(&value.to_le_bytes());
    }
    wav
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Укажите новый каталог для контрольного пакета")?,
    );
    if let Some(parent) = root.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir(&root)?;
    let png = picture()?;
    let wav = tone();
    let mut zip = ZipWriter::new(File::create(root.join("acceptance.zip"))?);
    for (name, bytes) in [
        ("main.dsl", DSL.as_bytes()),
        ("main_abrv.dsl", "n.\n существительное\n".as_bytes()),
        ("media/picture.png", png.as_slice()),
        ("media/tone.wav", wav.as_slice()),
        ("AUX.png", png.as_slice()),
    ] {
        zip.start_file(name, SimpleFileOptions::default())?;
        zip.write_all(bytes)?;
    }
    zip.finish()?;
    std::fs::write(root.join("picture.png"), png)?;
    std::fs::write(root.join("tone.wav"), wav)?;
    println!("{}", root.join("acceptance.zip").display());
    Ok(())
}
