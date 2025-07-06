pub use self::MarketType::*;
pub use self::news::*;
pub use crate::chart::*;
pub use crate::quote::models::*;

use chrono::Duration;
use iso_currency::Currency;
use serde::{Deserialize, Deserializer, Serialize};
use std::{collections::HashMap, fmt::Display};
pub mod news;
pub mod pine_indicator;

pub trait MarketSymbol {
    fn new<S: Into<String>>(symbol: S, exchange: S) -> Self;
    fn symbol(&self) -> &str;
    fn exchange(&self) -> &str;
    fn id(&self) -> String {
        format!("{}:{}", self.exchange(), self.symbol())
    }
}

impl MarketSymbol for Symbol {
    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn exchange(&self) -> &str {
        &self.exchange
    }
    fn new<S: Into<String>>(symbol: S, exchange: S) -> Self {
        Self {
            symbol: symbol.into(),
            exchange: exchange.into(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChartDrawing {
    pub success: bool,
    pub payload: ChartDrawingSource,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChartDrawingSource {
    pub sources: HashMap<String, ChartDrawingSourceData>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartDrawingSourceData {
    id: String,
    symbol: String,
    currency_id: String,
    server_update_time: i64,
    state: ChartDrawingSourceState,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartDrawingSourceState {
    points: Vec<ChartDrawingSourceStatePoint>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChartDrawingSourceStatePoint {
    time_t: i64,
    offset: i64,
    price: f64,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct UserCookies {
    pub id: u32,
    pub username: String,
    pub private_channel: String,
    pub auth_token: String,
    #[serde(default)]
    pub session: String,
    #[serde(default)]
    pub session_signature: String,
    pub session_hash: String,
    #[serde(default)]
    pub device_token: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SymbolSearchResponse {
    #[serde(rename(deserialize = "symbols_remaining"))]
    pub remaining: u64,
    pub symbols: Vec<Symbol>,
}

#[derive(Clone, PartialEq, Deserialize, Serialize, Debug, Default, Hash)]
pub struct Symbol {
    pub symbol: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, rename(deserialize = "type"))]
    pub market_type: String,
    #[serde(default)]
    pub exchange: String,
    #[serde(default)]
    pub currency_code: String,
    #[serde(default, rename(deserialize = "provider_id"))]
    pub data_provider: String,
    #[serde(default, rename(deserialize = "country"))]
    pub country_code: String,
    #[serde(default, rename(deserialize = "typespecs"))]
    pub type_specs: Vec<String>,
    #[serde(default, rename(deserialize = "source2"))]
    pub exchange_source: ExchangeSource,
}

#[bon::bon]
impl Symbol {
    #[builder]
    pub fn new<S: Into<String>>(symbol: S, exchange: S, currency: Option<Currency>) -> Self {
        Self {
            symbol: symbol.into(),
            exchange: exchange.into(),
            currency_code: currency.map(|c| c.to_string()).unwrap_or_default(),
            ..Default::default()
        }
    }

    pub fn id(&self) -> String {
        format!("{}:{}", self.exchange, self.symbol)
    }
}

#[derive(Clone, PartialEq, Deserialize, Serialize, Debug, Default, Hash)]
pub struct ExchangeSource {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash)]
pub enum SessionType {
    #[default]
    Regular,
    Extended,
    PreMarket,
    PostMarket,
}

impl Display for SessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionType::Regular => write!(f, "regular"),
            SessionType::Extended => write!(f, "extended"),
            SessionType::PreMarket => write!(f, "premarket"),
            SessionType::PostMarket => write!(f, "postmarket"),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy)]
pub enum MarketAdjustment {
    #[default]
    Splits,
    Dividends,
}

impl Display for MarketAdjustment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketAdjustment::Splits => write!(f, "splits"),
            MarketAdjustment::Dividends => write!(f, "dividends"),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy)]
pub enum MarketStatus {
    Holiday,
    #[default]
    Open,
    Close,
    Post,
    Pre,
}

impl Display for MarketStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketStatus::Holiday => write!(f, "holiday"),
            MarketStatus::Open => write!(f, "market"),
            MarketStatus::Close => write!(f, "out_of_session"),
            MarketStatus::Post => write!(f, "post_market"),
            MarketStatus::Pre => write!(f, "pre_market"),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash)]
pub enum Timezone {
    AfricaCairo,
    AfricaCasablanca,
    AfricaJohannesburg,
    AfricaLagos,
    AfricaNairobi,
    AfricaTunis,
    AmericaAnchorage,
    AmericaArgentinaBuenosAires,
    AmericaBogota,
    AmericaCaracas,
    AmericaChicago,
    AmericaElSalvador,
    AmericaJuneau,
    AmericaLima,
    AmericaLosAngeles,
    AmericaMexicoCity,
    AmericaNewYork,
    AmericaPhoenix,
    AmericaSantiago,
    AmericaSaoPaulo,
    AmericaToronto,
    AmericaVancouver,
    AsiaAlmaty,
    AsiaAshkhabad,
    AsiaBahrain,
    AsiaBangkok,
    AsiaChongqing,
    AsiaColombo,
    AsiaDhaka,
    AsiaDubai,
    AsiaHoChiMinh,
    AsiaHongKong,
    AsiaJakarta,
    AsiaJerusalem,
    AsiaKarachi,
    AsiaKathmandu,
    AsiaKolkata,
    AsiaKuwait,
    AsiaManila,
    AsiaMuscat,
    AsiaNicosia,
    AsiaQatar,
    AsiaRiyadh,
    AsiaSeoul,
    AsiaShanghai,
    AsiaSingapore,
    AsiaTaipei,
    AsiaTehran,
    AsiaTokyo,
    AsiaYangon,
    AtlanticReykjavik,
    AustraliaAdelaide,
    AustraliaBrisbane,
    AustraliaPerth,
    AustraliaSydney,
    EuropeAmsterdam,
    EuropeAthens,
    EuropeBelgrade,
    EuropeBerlin,
    EuropeBratislava,
    EuropeBrussels,
    EuropeBucharest,
    EuropeBudapest,
    EuropeCopenhagen,
    EuropeDublin,
    EuropeHelsinki,
    EuropeIstanbul,
    EuropeLisbon,
    EuropeLondon,
    EuropeLuxembourg,
    EuropeMadrid,
    EuropeMalta,
    EuropeMoscow,
    EuropeOslo,
    EuropeParis,
    EuropeRiga,
    EuropeRome,
    EuropeStockholm,
    EuropeTallinn,
    EuropeVilnius,
    EuropeWarsaw,
    EuropeZurich,
    PacificAuckland,
    PacificChatham,
    PacificFakaofo,
    PacificHonolulu,
    PacificNorfolk,
    USMountain,
    #[default]
    EtcUTC,
}

impl Display for Timezone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Timezone::AfricaCairo => write!(f, "Africa/Cairo"),
            Timezone::AfricaCasablanca => write!(f, "Africa/Casablanca"),
            Timezone::AfricaJohannesburg => write!(f, "Africa/Johannesburg"),
            Timezone::AfricaLagos => write!(f, "Africa/Lagos"),
            Timezone::AfricaNairobi => write!(f, "Africa/Nairobi"),
            Timezone::AfricaTunis => write!(f, "Africa/Tunis"),
            Timezone::AmericaAnchorage => write!(f, "America/Anchorage"),
            Timezone::AmericaArgentinaBuenosAires => write!(f, "America/Argentina/Buenos_Aires"),
            Timezone::AmericaBogota => write!(f, "America/Bogota"),
            Timezone::AmericaCaracas => write!(f, "America/Caracas"),
            Timezone::AmericaChicago => write!(f, "America/Chicago"),
            Timezone::AmericaElSalvador => write!(f, "America/El_Salvador"),
            Timezone::AmericaJuneau => write!(f, "America/Juneau"),
            Timezone::AmericaLima => write!(f, "America/Lima"),
            Timezone::AmericaLosAngeles => write!(f, "America/Los_Angeles"),
            Timezone::AmericaMexicoCity => write!(f, "America/Mexico_City"),
            Timezone::AmericaNewYork => write!(f, "America/New_York"),
            Timezone::AmericaPhoenix => write!(f, "America/Phoenix"),
            Timezone::AmericaSantiago => write!(f, "America/Santiago"),
            Timezone::AmericaSaoPaulo => write!(f, "America/Sao_Paulo"),
            Timezone::AmericaToronto => write!(f, "America/Toronto"),
            Timezone::AmericaVancouver => write!(f, "America/Vancouver"),
            Timezone::AsiaAlmaty => write!(f, "Asia/Almaty"),
            Timezone::AsiaAshkhabad => write!(f, "Asia/Ashkhabad"),
            Timezone::AsiaBahrain => write!(f, "Asia/Bahrain"),
            Timezone::AsiaBangkok => write!(f, "Asia/Bangkok"),
            Timezone::AsiaChongqing => write!(f, "Asia/Chongqing"),
            Timezone::AsiaColombo => write!(f, "Asia/Colombo"),
            Timezone::AsiaDhaka => write!(f, "Asia/Dhaka"),
            Timezone::AsiaDubai => write!(f, "Asia/Dubai"),
            Timezone::AsiaHoChiMinh => write!(f, "Asia/Ho_Chi_Minh"),
            Timezone::AsiaHongKong => write!(f, "Asia/Hong_Kong"),
            Timezone::AsiaJakarta => write!(f, "Asia/Jakarta"),
            Timezone::AsiaJerusalem => write!(f, "Asia/Jerusalem"),
            Timezone::AsiaKarachi => write!(f, "Asia/Karachi"),
            Timezone::AsiaKathmandu => write!(f, "Asia/Kathmandu"),
            Timezone::AsiaKolkata => write!(f, "Asia/Kolkata"),
            Timezone::AsiaKuwait => write!(f, "Asia/Kuwait"),
            Timezone::AsiaManila => write!(f, "Asia/Manila"),
            Timezone::AsiaMuscat => write!(f, "Asia/Muscat"),
            Timezone::AsiaNicosia => write!(f, "Asia/Nicosia"),
            Timezone::AsiaQatar => write!(f, "Asia/Qatar"),
            Timezone::AsiaRiyadh => write!(f, "Asia/Riyadh"),
            Timezone::AsiaSeoul => write!(f, "Asia/Seoul"),
            Timezone::AsiaShanghai => write!(f, "Asia/Shanghai"),
            Timezone::AsiaSingapore => write!(f, "Asia/Singapore"),
            Timezone::AsiaTaipei => write!(f, "Asia/Taipei"),
            Timezone::AsiaTehran => write!(f, "Asia/Tehran"),
            Timezone::AsiaTokyo => write!(f, "Asia/Tokyo"),
            Timezone::AsiaYangon => write!(f, "Asia/Yangon"),
            Timezone::AtlanticReykjavik => write!(f, "Atlantic/Reykjavik"),
            Timezone::AustraliaAdelaide => write!(f, "Australia/Adelaide"),
            Timezone::AustraliaBrisbane => write!(f, "Australia/Brisbane"),
            Timezone::AustraliaPerth => write!(f, "Australia/Perth"),
            Timezone::AustraliaSydney => write!(f, "Australia/Sydney"),
            Timezone::EuropeAmsterdam => write!(f, "Europe/Amsterdam"),
            Timezone::EuropeAthens => write!(f, "Europe/Athens"),
            Timezone::EuropeBelgrade => write!(f, "Europe/Belgrade"),
            Timezone::EuropeBerlin => write!(f, "Europe/Berlin"),
            Timezone::EuropeBratislava => write!(f, "Europe/Bratislava"),
            Timezone::EuropeBrussels => write!(f, "Europe/Brussels"),
            Timezone::EuropeBucharest => write!(f, "Europe/Bucharest"),
            Timezone::EuropeBudapest => write!(f, "Europe/Budapest"),
            Timezone::EuropeCopenhagen => write!(f, "Europe/Copenhagen"),
            Timezone::EuropeDublin => write!(f, "Europe/Dublin"),
            Timezone::EuropeHelsinki => write!(f, "Europe/Helsinki"),
            Timezone::EuropeIstanbul => write!(f, "Europe/Istanbul"),
            Timezone::EuropeLisbon => write!(f, "Europe/Lisbon"),
            Timezone::EuropeLondon => write!(f, "Europe/London"),
            Timezone::EuropeLuxembourg => write!(f, "Europe/Luxembourg"),
            Timezone::EuropeMadrid => write!(f, "Europe/Madrid"),
            Timezone::EuropeMalta => write!(f, "Europe/Malta"),
            Timezone::EuropeMoscow => write!(f, "Europe/Moscow"),
            Timezone::EuropeOslo => write!(f, "Europe/Oslo"),
            Timezone::EuropeParis => write!(f, "Europe/Paris"),
            Timezone::EuropeRiga => write!(f, "Europe/Riga"),
            Timezone::EuropeRome => write!(f, "Europe/Rome"),
            Timezone::EuropeStockholm => write!(f, "Europe/Stockholm"),
            Timezone::EuropeTallinn => write!(f, "Europe/Tallinn"),
            Timezone::EuropeVilnius => write!(f, "Europe/Vilnius"),
            Timezone::EuropeWarsaw => write!(f, "Europe/Warsaw"),
            Timezone::EuropeZurich => write!(f, "Europe/Zurich"),
            Timezone::PacificAuckland => write!(f, "Pacific/Auckland"),
            Timezone::PacificChatham => write!(f, "Pacific/Chatham"),
            Timezone::PacificFakaofo => write!(f, "Pacific/Fakaofo"),
            Timezone::PacificHonolulu => write!(f, "Pacific/Honolulu"),
            Timezone::PacificNorfolk => write!(f, "Pacific/Norfolk"),
            Timezone::USMountain => write!(f, "US/Mountain"),
            Timezone::EtcUTC => write!(f, "Etc/UTC"),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash)]
pub enum Interval {
    OneSecond = 0,
    FiveSeconds = 1,
    TenSeconds = 2,
    FifteenSeconds = 3,
    ThirtySeconds = 4,
    OneMinute = 5,
    ThreeMinutes = 6,
    FiveMinutes = 7,
    FifteenMinutes = 8,
    ThirtyMinutes = 9,
    FortyFiveMinutes = 10,
    OneHour = 11,
    TwoHours = 12,
    FourHours = 13,
    #[default]
    OneDay = 14,
    OneWeek = 15,
    OneMonth = 16,
    OneQuarter = 17,
    SixMonths = 18,
    Yearly = 19,
}

impl Interval {
    pub fn longer(self) -> Interval {
        match self {
            Interval::OneSecond => Interval::FiveSeconds,
            Interval::FiveSeconds => Interval::TenSeconds,
            Interval::TenSeconds => Interval::FifteenSeconds,
            Interval::FifteenSeconds => Interval::ThirtySeconds,
            Interval::ThirtySeconds => Interval::OneMinute,
            Interval::OneMinute => Interval::ThreeMinutes,
            Interval::ThreeMinutes => Interval::FiveMinutes,
            Interval::FiveMinutes => Interval::FifteenMinutes,
            Interval::FifteenMinutes => Interval::ThirtyMinutes,
            Interval::ThirtyMinutes => Interval::FortyFiveMinutes,
            Interval::FortyFiveMinutes => Interval::OneHour,
            Interval::OneHour => Interval::TwoHours,
            Interval::TwoHours => Interval::FourHours,
            Interval::FourHours => Interval::OneDay,
            Interval::OneDay => Interval::OneWeek,
            Interval::OneWeek => Interval::OneMonth,
            Interval::OneMonth => Interval::OneQuarter,
            Interval::OneQuarter => Interval::SixMonths,
            _ => self, // Yearly remains the same
        }
    }
}

impl From<u8> for Interval {
    fn from(value: u8) -> Self {
        match value {
            0 => Interval::OneSecond,
            1 => Interval::FiveSeconds,
            2 => Interval::TenSeconds,
            3 => Interval::FifteenSeconds,
            4 => Interval::ThirtySeconds,
            5 => Interval::OneMinute,
            6 => Interval::ThreeMinutes,
            7 => Interval::FiveMinutes,
            8 => Interval::FifteenMinutes,
            9 => Interval::ThirtyMinutes,
            10 => Interval::FortyFiveMinutes,
            11 => Interval::OneHour,
            12 => Interval::TwoHours,
            13 => Interval::FourHours,
            14 => Interval::OneDay,
            15 => Interval::OneWeek,
            16 => Interval::OneMonth,
            17 => Interval::OneQuarter,
            18 => Interval::SixMonths,
            _ => Interval::Yearly, // Default to Yearly for any other value
        }
    }
}

impl From<Interval> for Duration {
    fn from(interval: Interval) -> Self {
        match interval {
            Interval::OneSecond => Duration::seconds(1),
            Interval::FiveSeconds => Duration::seconds(5),
            Interval::TenSeconds => Duration::seconds(10),
            Interval::FifteenSeconds => Duration::seconds(15),
            Interval::ThirtySeconds => Duration::seconds(30),
            Interval::OneMinute => Duration::minutes(1),
            Interval::ThreeMinutes => Duration::minutes(3),
            Interval::FiveMinutes => Duration::minutes(5),
            Interval::FifteenMinutes => Duration::minutes(15),
            Interval::ThirtyMinutes => Duration::minutes(30),
            Interval::FortyFiveMinutes => Duration::minutes(45),
            Interval::OneHour => Duration::hours(1),
            Interval::TwoHours => Duration::hours(2),
            Interval::FourHours => Duration::hours(4),
            Interval::OneDay => Duration::days(1),
            Interval::OneWeek => Duration::weeks(1),
            Interval::OneMonth => Duration::days(30), // Approximation
            Interval::OneQuarter => Duration::days(90), // Approximation
            Interval::SixMonths => Duration::days(180), // Approximation
            Interval::Yearly => Duration::days(365),  // Approximation
        }
    }
}

impl From<&str> for Interval {
    fn from(value: &str) -> Self {
        match value {
            "1s" => Interval::OneSecond,
            "5s" => Interval::FiveSeconds,
            "10s" => Interval::TenSeconds,
            "15s" => Interval::FifteenSeconds,
            "30s" => Interval::ThirtySeconds,
            "1m" => Interval::OneMinute,
            "3m" => Interval::ThreeMinutes,
            "5m" => Interval::FiveMinutes,
            "15m" => Interval::FifteenMinutes,
            "30m" => Interval::ThirtyMinutes,
            "45m" => Interval::FortyFiveMinutes,
            "1h" => Interval::OneHour,
            "2h" => Interval::TwoHours,
            "4h" => Interval::FourHours,
            "1d" => Interval::OneDay,
            "7d" => Interval::OneWeek,
            "30d" => Interval::OneMonth,
            "120d" => Interval::OneQuarter,
            "180d" => Interval::SixMonths,
            "1y" => Interval::Yearly,
            _ => Interval::OneDay,
        }
    }
}

impl Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let time_interval = match self {
            Interval::OneSecond => "1S",
            Interval::FiveSeconds => "5S",
            Interval::TenSeconds => "10S",
            Interval::FifteenSeconds => "15S",
            Interval::ThirtySeconds => "30S",
            Interval::OneMinute => "1",
            Interval::ThreeMinutes => "3",
            Interval::FiveMinutes => "5",
            Interval::FifteenMinutes => "15",
            Interval::ThirtyMinutes => "30",
            Interval::FortyFiveMinutes => "45",
            Interval::OneHour => "1H",
            Interval::TwoHours => "2H",
            Interval::FourHours => "4H",
            Interval::OneDay => "1D",
            Interval::OneWeek => "1W",
            Interval::OneMonth => "1M",
            Interval::OneQuarter => "3M",
            Interval::SixMonths => "6M",
            Interval::Yearly => "12M",
        };
        write!(f, "{time_interval}")
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy)]
pub enum LanguageCode {
    Arabic,
    Chinese,
    Czech,
    Danish,
    Catalan,
    Dutch,
    #[default]
    English,
    Estonian,
    French,
    German,
    Greek,
    Hebrew,
    Hungarian,
    Indonesian,
    Italian,
    Japanese,
    Korean,
    Persian,
    Polish,
    Portuguese,
    Romanian,
    Russian,
    Slovak,
    Spanish,
    Swedish,
    Thai,
    Turkish,
    Vietnamese,
    Norwegian,
    Malay,
    TraditionalChinese,
}

impl Display for LanguageCode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            LanguageCode::Arabic => write!(f, "ar"),
            LanguageCode::Chinese => write!(f, "zh"),
            LanguageCode::Czech => write!(f, "cs"),
            LanguageCode::Danish => write!(f, "da_DK"),
            LanguageCode::Catalan => write!(f, "ca_ES"),
            LanguageCode::Dutch => write!(f, "nl_NL"),
            LanguageCode::English => write!(f, "en"),
            LanguageCode::Estonian => write!(f, "et_EE"),
            LanguageCode::French => write!(f, "fr"),
            LanguageCode::German => write!(f, "de"),
            LanguageCode::Greek => write!(f, "el"),
            LanguageCode::Hebrew => write!(f, "he_IL"),
            LanguageCode::Hungarian => write!(f, "hu_HU"),
            LanguageCode::Indonesian => write!(f, "id_ID"),
            LanguageCode::Italian => write!(f, "it"),
            LanguageCode::Japanese => write!(f, "ja"),
            LanguageCode::Korean => write!(f, "ko"),
            LanguageCode::Persian => write!(f, "fa"),
            LanguageCode::Polish => write!(f, "pl"),
            LanguageCode::Portuguese => write!(f, "pt"),
            LanguageCode::Romanian => write!(f, "ro"),
            LanguageCode::Russian => write!(f, "ru"),
            LanguageCode::Slovak => write!(f, "sk_SK"),
            LanguageCode::Spanish => write!(f, "es"),
            LanguageCode::Swedish => write!(f, "sv"),
            LanguageCode::Thai => write!(f, "th"),
            LanguageCode::Turkish => write!(f, "tr"),
            LanguageCode::Vietnamese => write!(f, "vi"),
            LanguageCode::Norwegian => write!(f, "no"),
            LanguageCode::Malay => write!(f, "ms_MY"),
            LanguageCode::TraditionalChinese => write!(f, "zh_TW"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum FinancialPeriod {
    FiscalYear,           // FY
    FiscalQuarter,        // FQ
    FiscalHalfYear,       // FH
    TrailingTwelveMonths, // TTM
    UnknownPeriod(String),
}

impl<'de> Deserialize<'de> for FinancialPeriod {
    fn deserialize<D>(deserializer: D) -> Result<FinancialPeriod, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        match s.as_str() {
            "FY" => Ok(FinancialPeriod::FiscalYear),
            "FQ" => Ok(FinancialPeriod::FiscalQuarter),
            "FH" => Ok(FinancialPeriod::FiscalHalfYear),
            "TTM" => Ok(FinancialPeriod::TrailingTwelveMonths),
            _ => Ok(FinancialPeriod::UnknownPeriod(s)),
        }
    }
}

impl Display for FinancialPeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            FinancialPeriod::FiscalYear => write!(f, "FY"),
            FinancialPeriod::FiscalQuarter => write!(f, "FQ"),
            FinancialPeriod::FiscalHalfYear => write!(f, "FH"),
            FinancialPeriod::TrailingTwelveMonths => write!(f, "TTM"),
            FinancialPeriod::UnknownPeriod(ref s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash)]
pub enum SymbolType {
    #[default]
    Stock,
    Index,
    Forex,
    Futures,
    Bitcoin,
    Crypto,
    Undefined,
    Expression,
    Spread,
    Cfd,
    Economic,
    Equity,
    Dr,
    Bond,
    Right,
    Warrant,
    Fund,
    Structured,
    Commodity,
    Fundamental,
    Spot,
}

impl Display for SymbolType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            SymbolType::Stock => write!(f, "stock"),
            SymbolType::Index => write!(f, "index"),
            SymbolType::Forex => write!(f, "forex"),
            SymbolType::Futures => write!(f, "futures"),
            SymbolType::Bitcoin => write!(f, "bitcoin"),
            SymbolType::Crypto => write!(f, "crypto"),
            SymbolType::Undefined => write!(f, "undefined"),
            SymbolType::Expression => write!(f, "expression"),
            SymbolType::Spread => write!(f, "spread"),
            SymbolType::Cfd => write!(f, "cfd"),
            SymbolType::Economic => write!(f, "economic"),
            SymbolType::Equity => write!(f, "equity"),
            SymbolType::Dr => write!(f, "dr"),
            SymbolType::Bond => write!(f, "bond"),
            SymbolType::Right => write!(f, "right"),
            SymbolType::Warrant => write!(f, "warrant"),
            SymbolType::Fund => write!(f, "fund"),
            SymbolType::Structured => write!(f, "structured"),
            SymbolType::Commodity => write!(f, "commodity"),
            SymbolType::Fundamental => write!(f, "fundamental"),
            SymbolType::Spot => write!(f, "spot"),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash)]
pub enum MarketType {
    #[default]
    All,
    Stocks(StocksType),
    Funds(FundsType),
    Futures,
    Forex,
    Crypto(CryptoType),
    Indices,
    Bonds,
    Economy,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash)]
pub enum StocksType {
    #[default]
    All,
    Common,
    Preferred,
    DepositoryReceipt,
    Warrant,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash)]
pub enum CryptoType {
    #[default]
    All,
    Spot,
    Futures,
    Swap,
    Index,
    Fundamental,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash)]
pub enum FundsType {
    #[default]
    All,
    ETF,
    MutualFund,
    Trust,
    REIT,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash)]
pub enum CryptoCentralization {
    #[default]
    CEX,
    DEX,
}

impl From<&str> for MarketType {
    fn from(value: &str) -> Self {
        match value {
            "all" | "undefined" => All,
            "stock" => Stocks(StocksType::All),
            "common_stock" => Stocks(StocksType::Common),
            "preferred_stock" => Stocks(StocksType::Preferred),
            "depository_receipt" => Stocks(StocksType::DepositoryReceipt),
            "warrant" => Stocks(StocksType::Warrant),
            "fund" => Funds(FundsType::All),
            "etf" => Funds(FundsType::ETF),
            "mutual_fund" => Funds(FundsType::MutualFund),
            "trust_fund" => Funds(FundsType::Trust),
            "reit" => Funds(FundsType::REIT),
            "futures" => Futures,
            "forex" => Forex,
            "crypto" => Crypto(CryptoType::All),
            "crypto_spot" => Crypto(CryptoType::Spot),
            "crypto_futures" => Crypto(CryptoType::Futures),
            "crypto_swap" => Crypto(CryptoType::Swap),
            "crypto_index" => Crypto(CryptoType::Index),
            "crypto_fundamental" => Crypto(CryptoType::Fundamental),
            "index" => Indices,
            "bond" => Bonds,
            "economic" => Economy,
            _ => All,
        }
    }
}

impl Display for MarketType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            All => write!(f, "undefined"),
            Stocks(t) => match t {
                StocksType::All => write!(f, "stocks"),
                StocksType::Common => write!(f, "common_stock"),
                StocksType::Preferred => write!(f, "preferred_stock"),
                StocksType::DepositoryReceipt => write!(f, "depository_receipt"),
                StocksType::Warrant => write!(f, "warrant"),
            },
            Funds(t) => match t {
                FundsType::All => write!(f, "funds"),
                FundsType::ETF => write!(f, "etf"),
                FundsType::MutualFund => write!(f, "mutual_fund"),
                FundsType::Trust => write!(f, "trust_fund"),
                FundsType::REIT => write!(f, "reit"),
            },
            Futures => write!(f, "futures"),
            Forex => write!(f, "forex"),
            Crypto(t) => match t {
                CryptoType::All => write!(f, "crypto"),
                CryptoType::Spot => write!(f, "crypto_spot"),
                CryptoType::Futures => write!(f, "crypto_futures"),
                CryptoType::Swap => write!(f, "crypto_swap"),
                CryptoType::Index => write!(f, "crypto_index"),
                CryptoType::Fundamental => write!(f, "crypto_fundamental"),
            },
            Indices => write!(f, "index"),
            Bonds => write!(f, "bond"),
            Economy => write!(f, "economic"),
        }
    }
}

impl Display for CryptoCentralization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            CryptoCentralization::CEX => write!(f, "cex"),
            CryptoCentralization::DEX => write!(f, "dex"),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy)]
pub enum FuturesProductType {
    SingleStock,
    WorldIndices,
    #[default]
    Currencies,
    InterestRates,
    Energy,
    Agriculture,
    Metals,
    Weather,
}

impl Display for FuturesProductType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            FuturesProductType::SingleStock => write!(f, "Financial%2FEquity"),
            FuturesProductType::WorldIndices => write!(f, "Financial%2FIndex"),
            FuturesProductType::Currencies => write!(f, "Financial%2FCurrency"),
            FuturesProductType::InterestRates => write!(f, "=Financial%2FInterestRate"),
            FuturesProductType::Energy => write!(f, "Financial%2FEnergy"),
            FuturesProductType::Agriculture => write!(f, "Financial%2FAgriculture"),
            FuturesProductType::Metals => write!(f, "Financial%2FMetals"),
            FuturesProductType::Weather => write!(f, "Financial%2FWeather"),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy)]
pub enum StockSector {
    CommercialServices,
    Communications,
    ConsumerDurables,
    ConsumerNonDurables,
    ConsumerServices,
    DistributionServices,
    ElectronicTechnology,
    EnergyMinerals,
    #[default]
    Finance,
    Government,
    HealthServices,
    HealthTechnology,
    IndustrialServices,
    Miscellaneous,
    NonEnergyMinerals,
    ProcessIndustries,
    ProducerManufacturing,
    RetailTrade,
    TechnologyServices,
    Transportation,
    Utilities,
}

impl Display for StockSector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            StockSector::CommercialServices => write!(f, "Commercial+Services"),
            StockSector::Communications => write!(f, "Communications"),
            StockSector::ConsumerDurables => write!(f, "Consumer+Durables"),
            StockSector::ConsumerNonDurables => write!(f, "Consumer+Non-Durables"),
            StockSector::ConsumerServices => write!(f, "Consumer+Services"),
            StockSector::DistributionServices => write!(f, "Distribution+Services"),
            StockSector::ElectronicTechnology => write!(f, "Electronic+Technology"),
            StockSector::EnergyMinerals => write!(f, "Energy+Minerals"),
            StockSector::Finance => write!(f, "Finance"),
            StockSector::Government => write!(f, "Government"),
            StockSector::HealthServices => write!(f, "Health+Services"),
            StockSector::HealthTechnology => write!(f, "Health+Technology"),
            StockSector::IndustrialServices => write!(f, "Industrial+Services"),
            StockSector::Miscellaneous => write!(f, "Miscellaneous"),
            StockSector::NonEnergyMinerals => write!(f, "Non-Energy+Minerals"),
            StockSector::ProcessIndustries => write!(f, "Process+Industries"),
            StockSector::ProducerManufacturing => write!(f, "Producer+Manufacturing"),
            StockSector::RetailTrade => write!(f, "Retail+Trade"),
            StockSector::TechnologyServices => write!(f, "Technology+Services"),
            StockSector::Transportation => write!(f, "Transportation"),
            StockSector::Utilities => write!(f, "Utilities"),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy)]
pub enum EconomicSource {
    #[default]
    WorldBank,
    EUROSTAT,
    AKAMAI,
    TransparencyInternational,
    OrganizationForEconomicCooperationAndDevelopment,
    WorldEconomicForum,
    WageIndicatorFoundation,
    BureauOfLaborStatistics,
    FederalReserve,
    StockholmInternationalPeaceResearchInstitute,
    InstituteForEconomicsAndPeace,
    BureauOfEconomicAnalysis,
    WorldGoldCouncil,
    CensusBureau,
    CentralBankOfWestAfricanStates,
    InternationalMonetaryFund,
    USEnergyInformationAdministration,
    StatisticCanada,
    OfficeForNationalStatistics,
    StatisticsNorway,
}

impl Display for EconomicSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            EconomicSource::WorldBank => write!(f, "__WB"),
            EconomicSource::EUROSTAT => write!(f, "__EUROSTAT"),
            EconomicSource::AKAMAI => write!(f, "__AKAMAI"),
            EconomicSource::TransparencyInternational => write!(f, "__TI"),
            EconomicSource::OrganizationForEconomicCooperationAndDevelopment => write!(f, "__OECD"),
            EconomicSource::WorldEconomicForum => write!(f, "__WEF"),
            EconomicSource::WageIndicatorFoundation => write!(f, "__WIF"),
            EconomicSource::BureauOfLaborStatistics => write!(f, "USBLS"),
            EconomicSource::FederalReserve => write!(f, "USFR"),
            EconomicSource::StockholmInternationalPeaceResearchInstitute => write!(f, "__SIPRI"),
            EconomicSource::InstituteForEconomicsAndPeace => write!(f, "__IEP"),
            EconomicSource::BureauOfEconomicAnalysis => write!(f, "USBEA"),
            EconomicSource::WorldGoldCouncil => write!(f, "__WGC"),
            EconomicSource::CensusBureau => write!(f, "USCB"),
            EconomicSource::CentralBankOfWestAfricanStates => write!(f, "__BCEAO"),
            EconomicSource::InternationalMonetaryFund => write!(f, "__IMF"),
            EconomicSource::USEnergyInformationAdministration => write!(f, "__UEIA"),
            EconomicSource::StatisticCanada => write!(f, "CASC"),
            EconomicSource::OfficeForNationalStatistics => write!(f, "GBONS"),
            EconomicSource::StatisticsNorway => write!(f, "NOSN"),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy)]
pub enum EconomicCategory {
    #[default]
    GDP,
    Labor,
    Prices,
    Health,
    Money,
    Trade,
    Government,
    Business,
    Consumer,
    Housing,
    Taxes,
}

impl Display for EconomicCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            EconomicCategory::GDP => write!(f, "gdp"),
            EconomicCategory::Labor => write!(f, "lbr"),
            EconomicCategory::Prices => write!(f, "prce"),
            EconomicCategory::Health => write!(f, "hlth"),
            EconomicCategory::Money => write!(f, "mny"),
            EconomicCategory::Trade => write!(f, "trd"),
            EconomicCategory::Government => write!(f, "gov"),
            EconomicCategory::Business => write!(f, "bsnss"),
            EconomicCategory::Consumer => write!(f, "cnsm"),
            EconomicCategory::Housing => write!(f, "hse"),
            EconomicCategory::Taxes => write!(f, "txs"),
        }
    }
}
