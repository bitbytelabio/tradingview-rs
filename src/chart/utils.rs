use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash)]
pub enum ExtendValue {
    Right,
    Left,
    Both,
    None,
}

#[derive(Debug, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash)]
pub enum YLocValue {
    Price,
    AboveBar,
    BelowBar,
}

#[derive(Debug, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash)]
pub enum LabelStyleValue {
    None,
    XCross,
    Cross,
    TriangleUp,
    TriangleDown,
    Flag,
    Circle,
    ArrowUp,
    ArrowDown,
    LabelUp,
    LabelDown,
    LabelLeft,
    LabelRight,
    LabelLowerLeft,
    LabelLowerRight,
    LabelUpperLeft,
    LabelUpperRight,
    LabelCenter,
    Square,
    Diamond,
}

#[derive(Debug, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash)]
pub enum LineStyleValue {
    Solid,
    Dotted,
    Dashed,
    ArrowLeft,
    ArrowRight,
    ArrowBoth,
}

#[derive(Debug, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash)]
pub enum BoxStyleValue {
    Solid,
    Dotted,
    Dashed,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct GraphicLabel {
    pub id: u64,
    pub x: Option<f64>,
    pub y: f64,
    pub y_loc: YLocValue,
    pub text: String,
    pub style: LabelStyleValue,
    pub color: u32,
    pub text_color: u32,
    pub size: String,
    pub text_align: String,
    pub tool_tip: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Copy, PartialEq)]
pub struct GraphicLine {
    pub id: u64,
    pub x1: Option<f64>,
    pub y1: f64,
    pub x2: Option<f64>,
    pub y2: f64,
    pub extend: ExtendValue,
    pub style: LineStyleValue,
    pub color: u32,
    pub width: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct GraphicBox {
    pub id: u64,
    pub x1: Option<f64>,
    pub y1: f64,
    pub x2: Option<f64>,
    pub y2: f64,
    pub color: u32,
    pub bg_color: u32,
    pub extend: ExtendValue,
    pub style: BoxStyleValue,
    pub width: f64,
    pub text: String,
    pub text_size: String,
    pub text_color: u32,
    pub text_v_align: String,
    pub text_h_align: String,
    pub text_wrap: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct GraphicData {
    pub labels: Vec<GraphicLabel>,
    pub lines: Vec<GraphicLine>,
    pub boxes: Vec<GraphicBox>,
}

fn translate_extend(value: &str) -> ExtendValue {
    match value {
        "r" => ExtendValue::Right,
        "l" => ExtendValue::Left,
        "b" => ExtendValue::Both,
        "n" => ExtendValue::None,
        _ => ExtendValue::None,
    }
}

fn translate_y_loc(value: &str) -> YLocValue {
    match value {
        "pr" => YLocValue::Price,
        "ab" => YLocValue::AboveBar,
        "bl" => YLocValue::BelowBar,
        _ => YLocValue::Price,
    }
}

fn translate_label_style(value: &str) -> LabelStyleValue {
    match value {
        "n" => LabelStyleValue::None,
        "xcr" => LabelStyleValue::XCross,
        "cr" => LabelStyleValue::Cross,
        "tup" => LabelStyleValue::TriangleUp,
        "tdn" => LabelStyleValue::TriangleDown,
        "flg" => LabelStyleValue::Flag,
        "cir" => LabelStyleValue::Circle,
        "aup" => LabelStyleValue::ArrowUp,
        "adn" => LabelStyleValue::ArrowDown,
        "lup" => LabelStyleValue::LabelUp,
        "ldn" => LabelStyleValue::LabelDown,
        "llf" => LabelStyleValue::LabelLeft,
        "lrg" => LabelStyleValue::LabelRight,
        "llwlf" => LabelStyleValue::LabelLowerLeft,
        "llwrg" => LabelStyleValue::LabelLowerRight,
        "luplf" => LabelStyleValue::LabelUpperLeft,
        "luprg" => LabelStyleValue::LabelUpperRight,
        "lcn" => LabelStyleValue::LabelCenter,
        "sq" => LabelStyleValue::Square,
        "dia" => LabelStyleValue::Diamond,
        _ => LabelStyleValue::None,
    }
}

fn translate_line_style(value: &str) -> LineStyleValue {
    match value {
        "sol" => LineStyleValue::Solid,
        "dot" => LineStyleValue::Dotted,
        "dsh" => LineStyleValue::Dashed,
        "al" => LineStyleValue::ArrowLeft,
        "ar" => LineStyleValue::ArrowRight,
        "ab" => LineStyleValue::ArrowBoth,
        _ => LineStyleValue::Solid,
    }
}

fn translate_box_style(value: &str) -> BoxStyleValue {
    match value {
        "sol" => BoxStyleValue::Solid,
        "dot" => BoxStyleValue::Dotted,
        "dsh" => BoxStyleValue::Dashed,
        _ => BoxStyleValue::Solid,
    }
}

pub fn graphics_parser(data: &Value) -> GraphicData {
    let indexes = data
        .get("indexes")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or(Vec::new());

    let raw_graphic = data.get("graphic").unwrap_or(&Value::Null);

    // Parse labels
    let labels = if let Some(dwglabels) = raw_graphic.get("dwglabels").and_then(|v| v.as_object()) {
        dwglabels
            .values()
            .filter_map(|l| {
                Some(GraphicLabel {
                    id: l.get("id")?.as_u64()?,
                    x: l.get("x")
                        .and_then(|x| x.as_u64())
                        .and_then(|idx| indexes.get(idx as usize))
                        .and_then(|v| v.as_f64()),
                    y: l.get("y")?.as_f64()?,
                    y_loc: translate_y_loc(l.get("yl")?.as_str()?),
                    text: l.get("t")?.as_str()?.to_string(),
                    style: translate_label_style(l.get("st")?.as_str()?),
                    color: l.get("ci")?.as_u64()? as u32,
                    text_color: l.get("tci")?.as_u64()? as u32,
                    size: l.get("sz")?.as_str()?.to_string(),
                    text_align: l.get("ta")?.as_str()?.to_string(),
                    tool_tip: l.get("tt")?.as_str()?.to_string(),
                })
            })
            .collect()
    } else {
        vec![]
    };

    // Parse lines
    let lines = if let Some(dwglines) = raw_graphic.get("dwglines").and_then(|v| v.as_object()) {
        dwglines
            .values()
            .filter_map(|l| {
                Some(GraphicLine {
                    id: l.get("id")?.as_u64()?,
                    x1: l
                        .get("x1")
                        .and_then(|x| x.as_u64())
                        .and_then(|idx| indexes.get(idx as usize))
                        .and_then(|v| v.as_f64()),
                    y1: l.get("y1")?.as_f64()?,
                    x2: l
                        .get("x2")
                        .and_then(|x| x.as_u64())
                        .and_then(|idx| indexes.get(idx as usize))
                        .and_then(|v| v.as_f64()),
                    y2: l.get("y2")?.as_f64()?,
                    extend: translate_extend(l.get("ex")?.as_str()?),
                    style: translate_line_style(l.get("st")?.as_str()?),
                    color: l.get("ci")?.as_u64()? as u32,
                    width: l.get("w")?.as_f64()?,
                })
            })
            .collect()
    } else {
        vec![]
    };

    // Parse boxes
    let boxes = if let Some(dwgboxes) = raw_graphic.get("dwgboxes").and_then(|v| v.as_object()) {
        dwgboxes
            .values()
            .filter_map(|b| {
                Some(GraphicBox {
                    id: b.get("id")?.as_u64()?,
                    x1: b
                        .get("x1")
                        .and_then(|x| x.as_u64())
                        .and_then(|idx| indexes.get(idx as usize))
                        .and_then(|v| v.as_f64()),
                    y1: b.get("y1")?.as_f64()?,
                    x2: b
                        .get("x2")
                        .and_then(|x| x.as_u64())
                        .and_then(|idx| indexes.get(idx as usize))
                        .and_then(|v| v.as_f64()),
                    y2: b.get("y2")?.as_f64()?,
                    color: b.get("c")?.as_u64()? as u32,
                    bg_color: b.get("bc")?.as_u64()? as u32,
                    extend: translate_extend(b.get("ex")?.as_str()?),
                    style: translate_box_style(b.get("st")?.as_str()?),
                    width: b.get("w")?.as_f64()?,
                    text: b.get("t")?.as_str()?.to_string(),
                    text_size: b.get("ts")?.as_str()?.to_string(),
                    text_color: b.get("tc")?.as_u64()? as u32,
                    text_v_align: b.get("tva")?.as_str()?.to_string(),
                    text_h_align: b.get("tha")?.as_str()?.to_string(),
                    text_wrap: b.get("tw")?.as_str()?.to_string(),
                })
            })
            .collect()
    } else {
        vec![]
    };

    GraphicData {
        labels,
        lines,
        boxes,
    }
}
