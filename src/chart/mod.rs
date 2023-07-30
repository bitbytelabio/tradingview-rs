use phf::phf_map;

mod graphic_parser;
mod study;
pub mod websocket;

#[derive(Debug, PartialEq, Clone)]
pub enum ChartEvent {}

static CHART_TYPES: phf::Map<&'static str, &'static str> = phf_map! {
    "HeikinAshi"=>"BarSetHeikenAshi@tv-basicstudies-60!",
    "Renko"=> "BarSetRenko@tv-prostudies-40!",
    "LineBreak"=> "BarSetPriceBreak@tv-prostudies-34!",
    "Kagi"=> "BarSetKagi@tv-prostudies-34!",
    "PointAndFigure"=> "BarSetPnF@tv-prostudies-34!",
    "Range"=> "BarSetRange@tv-basicstudies-72!",
};
