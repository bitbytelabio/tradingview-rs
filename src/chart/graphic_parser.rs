use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Translator {
    extend: HashMap<char, String>,
    y_loc: HashMap<String, String>,
    label_style: HashMap<String, String>,
    line_style: HashMap<String, String>,
    box_style: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Label {
    id: String,
    x: i64,
    y: f64,
    y_loc: String,
    text: String,
    style: String,
    color: String,
    text_color: String,
    size: f64,
    text_align: String,
    tool_tip: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Line {
    id: String,
    x1: i64,
    y1: f64,
    x2: i64,
    y2: f64,
    extend: String,
    style: String,
    color: String,
    width: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Box {
    id: String,
    x1: i64,
    y1: f64,
    x2: i64,
    y2: f64,
    color: String,
    bg_color: String,
    extend: String,
    style: String,
    width: f64,
    text: String,
    text_size: f64,
    text_color: String,
    text_v_align: String,
    text_h_align: String,
    text_wrap: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Cell {
    id: String,
    text: String,
    width: f64,
    height: f64,
    text_color: String,
    text_h_align: String,
    text_v_align: String,
    text_size: f64,
    bg_color: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Table {
    id: String,
    position: String,
    rows: i64,
    columns: i64,
    bg_color: String,
    frame_color: String,
    frame_width: f64,
    border_color: String,
    border_width: f64,
    cells: Vec<Vec<Option<Cell>>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct HorizLine {
    id: String,
    y: f64,
    start_index: i64,
    end_index: i64,
    color: String,
    width: f64,
    style: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Point {
    index: i64,
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Polygon {
    id: String,
    points: Vec<Point>,
    color: String,
    bg_color: String,
    width: f64,
    style: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct HorizHist {
    id: String,
    first_bar_time: i64,
    last_bar_time: i64,
    y: f64,
    color: String,
    width: f64,
    style: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct GraphicDataResponse {
    dwglabels: HashMap<String, Label>,
    dwglines: HashMap<String, Line>,
    dwgboxes: HashMap<String, Box>,
    dwgtables: HashMap<String, Table>,
    dwgtablecells: HashMap<String, Cell>,
    horizlines: HashMap<String, HorizLine>,
    polygons: HashMap<String, Polygon>,
    hhists: HashMap<String, HorizHist>,
}

fn graphic_parse(raw_graphic: GraphicDataResponse, indexes: Vec<i64>) -> HashMap<String, Vec<Box>> {
    let translator = Translator {
        extend: [('r', "right"), ('l', "left"), ('b', "both"), ('n', "none")]
            .iter()
            .cloned()
            .map(|(k, v)| (k, v.to_string()))
            .collect(),
        y_loc: [("pr", "price"), ("ab", "abovebar"), ("bl", "belowbar")]
            .iter()
            .cloned()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        label_style: [
            ("n", "none"),
            ("xcr", "xcross"),
            ("cr", "cross"),
            ("tup", "triangleup"),
            ("tdn", "triangledown"),
            ("flg", "flag"),
            ("cir", "circle"),
            ("aup", "arrowup"),
            ("adn", "arrowdown"),
            ("lup", "label_up"),
            ("ldn", "label_down"),
            ("llf", "label_left"),
            ("lrg", "label_right"),
            ("llwlf", "label_lower_left"),
            ("llwrg", "label_lower_right"),
            ("luplf", "label_upper_left"),
            ("luprg", "label_upper_right"),
            ("lcn", "label_center"),
            ("sq", "square"),
            ("dia", "diamond"),
        ]
        .iter()
        .cloned()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect(),
        line_style: [
            ("sol", "solid"),
            ("dot", "dotted"),
            ("dsh", "dashed"),
            ("al", "arrow_left"),
            ("ar", "arrow_right"),
            ("ab", "arrow_both"),
        ]
        .iter()
        .cloned()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect(),
        box_style: [("sol", "solid"), ("dot", "dotted"), ("dsh", "dashed")]
            .iter()
            .cloned()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    };

    let mut boxes: HashMap<String, Vec<Box>> = HashMap::new();

    for (id, label) in raw_graphic.dwglabels {
        let boxy = Box {
            id: label.id,
            x1: indexes[label.x as usize],
            y1: label.y,
            x2: 0,
            y2: 0.0,
            color: label.color,
            bg_color: "".to_string(),
            extend: "".to_string(),
            style: "".to_string(),
            width: 0.0,
            text: label.text,
            text_size: label.size,
            text_color: label.text_color,
            text_v_align: "".to_string(),
            text_h_align: label.text_align,
            text_wrap: "".to_string(),
        };
        boxes.entry(id).or_insert_with(Vec::new).push(boxy);
    }

    for (id, line) in raw_graphic.dwglines {
        let boxy = Box {
            id: line.id,
            x1: indexes[line.x1 as usize],
            y1: line.y1,
            x2: indexes[line.x2 as usize],
            y2: line.y2,
            color: line.color,
            bg_color: "".to_string(),
            extend: translator.extend[&line.extend.chars().next().unwrap()].clone(),
            style: translator.line_style[&line.style].clone(),
            width: line.width,
            text: "".to_string(),
            text_size: 0.0,
            text_color: "".to_string(),
            text_v_align: "".to_string(),
            text_h_align: "".to_string(),
            text_wrap: "".to_string(),
        };
        boxes.entry(id).or_insert_with(Vec::new).push(boxy);
    }

    for (id, b) in raw_graphic.dwgboxes {
        let boxy = Box {
            id: b.id,
            x1: indexes[b.x1 as usize],
            y1: b.y1,
            x2: indexes[b.x2 as usize],
            y2: b.y2,
            color: b.color,
            bg_color: b.bg_color,
            extend: translator.extend[&b.extend.chars().next().unwrap()].clone(),
            style: translator.box_style[&b.style].clone(),
            width: b.width,
            text: b.text,
            text_size: b.text_size,
            text_color: b.text_color,
            text_v_align: b.text_v_align,
            text_h_align: b.text_h_align,
            text_wrap: b.text_wrap,
        };
        boxes.entry(id).or_insert_with(Vec::new).push(boxy);
    }

    boxes
}
