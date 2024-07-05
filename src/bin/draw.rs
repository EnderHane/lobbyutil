use std::{collections::BTreeMap, fs::OpenOptions, mem::replace, path::PathBuf};

use euclid::{Angle, Point2D, Rotation2D};
use itertools::Itertools;
use parley::{
    fontique::{Collection, CollectionOptions},
    layout::Alignment,
    style::{FontFamily, StyleProperty},
    swash::{
        scale::outline::Outline,
        zeno::{Command, PathData},
    },
    FontContext, Layout, LayoutContext,
};
use parley::{
    style::FontStack,
    swash::{scale::ScaleContext, FontRef},
};
use test_celesteloader::PNG_MAGIC_STR;
use tiny_skia::{
    Color as TinySkiaColor, FillRule, IntSize, LineCap, LineJoin, Paint, Path as SkiaPath,
    PathBuilder, Pixmap, Shader, Stroke, Transform as TinySkiaTransform,
};

const DIGIT_FONT: &[u8] = include_bytes!("../../SourceHanSansSC-Bold-subset.otf");
const EMOJI_FONT: &[u8] = include_bytes!("../../NotoEmoji-VariableFont_wght-subset.ttf");
const MATH_FONT: &[u8] = include_bytes!("../../NotoSansMath-Regular-subset.otf");

// const _SUB: &str = "123456ABC♥↺";

struct TextManager {
    font_ctx: FontContext,
    layout_ctx: LayoutContext,
    scale_ctx: ScaleContext,
    font_family_names: Vec<String>,
}

impl TextManager {
    fn new(system_fonts: bool) -> Self {
        let font_ctx = FontContext {
            collection: Collection::new(CollectionOptions {
                system_fonts,
                ..Default::default()
            }),
            ..Default::default()
        };
        let layout_ctx = LayoutContext::new();
        let scale_ctx = ScaleContext::new();

        Self {
            font_ctx,
            layout_ctx,
            scale_ctx,
            font_family_names: Vec::default(),
        }
    }
}

impl TextManager {
    fn add_font(&mut self, data: Vec<u8>) {
        let r = self.font_ctx.collection.register_fonts(data);
        for (fid, _) in r {
            let ftn = self.font_ctx.collection.family_name(fid).unwrap();
            self.font_family_names.push(ftn.into());
        }
    }

    fn build_layout(
        &mut self,
        text: &str,
        font_size: f32,
        max_advance: Option<f32>,
        align: Option<Alignment>,
        scale: Option<f32>,
        line_height: Option<f32>,
        rgba: impl Into<[u8; 4]>,
    ) -> Layout<[u8; 4]> {
        let mut builder =
            self.layout_ctx
                .ranged_builder(&mut self.font_ctx, text, scale.unwrap_or(1.0));
        let ffns = self
            .font_family_names
            .iter()
            .map(String::as_str)
            .map(FontFamily::Named)
            .collect::<Vec<_>>();
        builder.push_default(&StyleProperty::FontStack(FontStack::List(&ffns)));
        builder.push_default(&StyleProperty::Brush(rgba.into()));
        builder.push_default(&StyleProperty::LineHeight(line_height.unwrap_or(1.0)));
        builder.push_default(&StyleProperty::FontSize(font_size));

        let mut layout = builder.build();
        layout.break_all_lines(max_advance, align.unwrap_or_default());
        layout
    }

    fn generate_paths<'l>(
        &'l mut self,
        layout: &'l Layout<[u8; 4]>,
    ) -> impl Iterator<Item = (Outline, (f32, f32), [u8; 4])> + 'l {
        layout
            .lines()
            .flat_map(|line| line.glyph_runs())
            .flat_map(|glyph_run| {
                let run_x = glyph_run.offset();
                let run_y = glyph_run.baseline();
                let style = glyph_run.style();
                let color = style.brush;
                let run = glyph_run.run();
                let font = run.font();
                let font_size = run.font_size();
                let normalized_coords = run.normalized_coords();
                let font_ref =
                    FontRef::from_index(font.data.as_ref(), font.index as usize).unwrap();
                let mut scaler = self
                    .scale_ctx
                    .builder(font_ref)
                    .size(font_size)
                    .hint(true)
                    .normalized_coords(normalized_coords)
                    .build();
                glyph_run
                    .glyphs()
                    .scan(run_x, |st, glyph| {
                        let path = scaler.scale_outline(glyph.id).unwrap();
                        let x = glyph.x + replace(st, *st + glyph.advance);
                        let y = glyph.y + run_y;
                        Some((path, (x, y), color))
                    })
                    .collect::<Vec<_>>()
            })
    }

    fn draw(
        &mut self,
        text: &str,
        font_size: f32,
        max_advance: Option<f32>,
        align: Option<Alignment>,
        scale: Option<f32>,
        line_height: Option<f32>,
        rgba: impl Into<[u8; 4]>,
        f: impl FnMut((Outline, (f32, f32), [u8; 4])),
    ) {
        let layout = self.build_layout(
            text,
            font_size,
            max_advance,
            align,
            scale,
            line_height,
            rgba,
        );
        self.generate_paths(&layout).for_each(f)
    }
}

fn convert_path(path_data: impl PathData) -> Option<SkiaPath> {
    let mut pb = PathBuilder::new();
    for cmd in path_data.commands() {
        match cmd {
            Command::MoveTo(p) => pb.move_to(p.x, p.y),
            Command::LineTo(p) => pb.line_to(p.x, p.y),
            Command::CurveTo(p1, p2, p) => pb.cubic_to(p1.x, p1.y, p2.x, p2.y, p.x, p.y),
            Command::QuadTo(p1, p) => pb.quad_to(p1.x, p1.y, p.x, p.y),
            Command::Close => pb.close(),
        }
    }
    pb.finish()
}

fn create_arrow(start: Point2D<f32, f32>, end: Point2D<f32, f32>) -> Option<SkiaPath> {
    let mut pb = PathBuilder::new();
    pb.move_to(start.x, start.y);
    pb.line_to(end.x, end.y);
    let mid = start.lerp(end, 0.5);
    let dire = (end - start).normalize();
    let pos_15_deg = Rotation2D::new(Angle::degrees(15.0));
    let neg_15_deg = Rotation2D::new(Angle::degrees(-15.0));
    let wing_length = 60.0;
    let wing1 = mid - pos_15_deg.transform_vector(dire) * wing_length;
    let wing2 = mid - neg_15_deg.transform_vector(dire) * wing_length;
    pb.move_to(mid.x, mid.y);
    pb.line_to(wing1.x, wing1.y);
    pb.move_to(mid.x, mid.y);
    pb.line_to(wing2.x, wing2.y);
    pb.finish()
}

#[derive(clap::Parser)]
struct Cli {
    #[arg(long, short)]
    input_png: PathBuf,
    output_file: PathBuf,
    #[arg(long = "json")]
    json_graph: Option<String>,
    #[arg(long = "hy")]
    hyphen_sep: Option<String>,
}

fn main() {
    let cli: Cli = clap::Parser::parse();

    let mut mgr = TextManager::new(false);
    mgr.add_font(DIGIT_FONT.into());
    mgr.add_font(EMOJI_FONT.into());
    mgr.add_font(MATH_FONT.into());

    let bg = OpenOptions::new().read(true).open(cli.input_png).unwrap();
    let bg_pic = png::Decoder::new(bg);
    let mut reader = bg_pic.read_info().unwrap();

    let draw_list = reader
        .info()
        .utf8_text
        .iter()
        .find(|chk| chk.keyword == PNG_MAGIC_STR)
        .unwrap()
        .get_text()
        .unwrap();

    let map: BTreeMap<&str, [f32; 2]> = serde_json::from_str(&draw_list).unwrap();

    let mut bg_buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut bg_buf).unwrap();

    let mut canvas =
        Pixmap::from_vec(bg_buf, IntSize::from_wh(info.width, info.height).unwrap()).unwrap();

    for (&text, &[x, y]) in &map {
        let font_size = 96.0;
        let transf = TinySkiaTransform::from_scale(1.0, -1.0).post_translate(x, y);
        let fill_color = if text.chars().all(|c| c.is_ascii_digit()) {
            TinySkiaColor::from_rgba8(255, 175, 195, 230)
        } else if text.chars().all(|c| c.is_ascii_alphabetic()) {
            TinySkiaColor::from_rgba8(150, 175, 255, 230)
        } else {
            TinySkiaColor::from_rgba8(255, 240, 100, 230)
        };
        let paint = Paint {
            shader: Shader::SolidColor(fill_color),
            ..Default::default()
        };
        let stroke_paint = Paint {
            shader: Shader::SolidColor(TinySkiaColor::from_rgba8(42, 12, 12, 250)),
            ..Default::default()
        };
        let stroke = Stroke {
            width: (font_size / 24f32).round(),
            miter_limit: (font_size / 24f32).ceil(),
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Default::default()
        };
        mgr.draw(
            text,
            font_size,
            None,
            None,
            None,
            Some(0.75),
            [0, 0, 0, 255],
            |(ol, (gx, gy), _)| {
                let path = convert_path(ol.path()).unwrap();
                let g_transf = transf.post_translate(gx, gy);
                canvas.fill_path(&path, &paint, FillRule::Winding, g_transf, None);
                canvas.stroke_path(&path, &stroke_paint, &stroke, g_transf, None);
            },
        );
    }

    fn conv_vert(v: &str) -> &str {
        if "0" > v {
            "↺"
        } else if ":" <= v && v < "A" {
            "♥"
        } else {
            v
        }
    }

    if let Some(s) = cli.json_graph {
        let graph: BTreeMap<&str, BTreeMap<&str, serde::de::IgnoredAny>> =
            serde_json::from_str(&s).unwrap();
        for (src, es) in &graph {
            let start = conv_vert(src);
            for (dst, _) in es {
                let end = conv_vert(dst);
                let p_start = map[start];
                let p_end = map[end];
                let path = create_arrow(p_start.into(), p_end.into()).unwrap();
                let paint = Paint {
                    shader: Shader::SolidColor(TinySkiaColor::from_rgba8(190, 190, 250, 180)),
                    ..Default::default()
                };
                let stroke = Stroke {
                    width: 4.0,
                    miter_limit: 4.0,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    ..Default::default()
                };
                canvas.stroke_path(&path, &paint, &stroke, TinySkiaTransform::identity(), None);
            }
        }
    }

    if let Some(s) = cli.hyphen_sep {
        s.split('-')
            .map(conv_vert)
            .tuple_windows::<(_, _)>()
            .for_each(|(start, end)| {
                let p_start = map[start];
                let p_end = map[end];
                let path = create_arrow(p_start.into(), p_end.into()).unwrap();
                let paint = Paint {
                    shader: Shader::SolidColor(TinySkiaColor::from_rgba8(120, 250, 120, 230)),
                    ..Default::default()
                };
                let stroke = Stroke {
                    width: 6.0,
                    miter_limit: 4.0,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    ..Default::default()
                };
                canvas.stroke_path(&path, &paint, &stroke, TinySkiaTransform::identity(), None);
            });
    }

    let output = OpenOptions::new()
        .write(true)
        .create(true)
        .open(cli.output_file)
        .unwrap();
    let mut out_png = png::Encoder::new(output, canvas.width(), canvas.height());
    out_png.set_color(png::ColorType::Rgba);
    out_png
        .write_header()
        .unwrap()
        .write_image_data(canvas.data())
        .unwrap();
}
