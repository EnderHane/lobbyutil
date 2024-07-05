use alphabet::{alphabet, Alphabet};
use anyhow::Result;
use celesteloader::{
    map::{decode::decode_map, load_map_from_element},
    CelesteInstallation,
};

use std::{collections::BTreeMap, path::PathBuf};

use test_celesteloader::{
    chapters, default_spwan, find_mini_heart_door, pos_bounded, pos_in_room, start_level, warps,
    PNG_MAGIC_STR,
};

alphabet!(pub ENGLISH = "ABCDEFGHIJKLMNOPQRSTUVWXYZ");

#[derive(clap::Parser)]
struct Cli {
    #[arg(long)]
    game: PathBuf,
    #[arg(long)]
    r#mod: String,
    #[arg(long)]
    map: String,
    #[arg(short, long, requires = "output_png")]
    input_png: Option<PathBuf>,
    #[arg(short, long, requires = "input_png")]
    output_png: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli: Cli = clap::Parser::parse();
    let celeste = CelesteInstallation { path: cli.game };
    let mut gal = celeste.read_mod(&cli.r#mod)?;
    let lob = gal.read_file(&format!("Maps/{}.bin", cli.map))?;
    let root = decode_map(&lob)?;
    let mp = load_map_from_element(&root)?;
    let start_room = start_level(&root, &mp);
    let default_spwan = default_spwan(&start_room);
    let chs = chapters(&mp);
    let warps = warps(&mp);

    let mut nodes = BTreeMap::new();
    let pos_b = pos_bounded(pos_in_room(default_spwan.position, start_room), &mp);
    let (x, y) = pos_b;
    nodes.insert("↺".to_string(), (x, y));
    for (i, &(ch, r)) in chs.iter().enumerate() {
        let pos_b = pos_bounded(pos_in_room(ch.position, r), &mp);
        let (x, y) = pos_b;
        nodes.insert(format!("{}", i + 1), (x, y));
    }
    for (i, &(w, r)) in warps.iter().enumerate() {
        let pos_b = pos_bounded(pos_in_room(w.position, r), &mp);
        let (x, y) = pos_b;
        nodes.insert(ENGLISH.iter_words().nth(i + 1).unwrap(), (x, y));
    }
    if let Some((door, r)) = find_mini_heart_door(&mp) {
        let pos_b = pos_bounded(pos_in_room(door.position, r), &mp);
        let (x, y) = pos_b;
        nodes.insert("♥".to_string(), (x, y));
    }
    let s = serde_json::to_string(&nodes).unwrap();
    println!("{s}");

    if let Some((in_path, out_path)) = cli.input_png.zip(cli.output_png) {
        let png_raw = std::fs::File::open(&in_path).unwrap();
        let mut p = png::Decoder::new(png_raw).read_info().unwrap();
        let (w, h) = (p.info().width, p.info().height);
        let mut buf = vec![0; p.output_buffer_size()];
        p.next_frame(&mut buf).unwrap();
        let png_out = std::fs::File::create(&out_path).unwrap();
        let mut o = png::Encoder::new(png_out, w, h);
        o.set_color(p.output_color_type().0);
        o.add_itxt_chunk(PNG_MAGIC_STR.into(), s).unwrap();
        o.write_header().unwrap().write_image_data(&buf).unwrap();
        eprintln!("written to iTXt \"{}\" in {}", PNG_MAGIC_STR, out_path.display());
    }

    Ok(())
}
