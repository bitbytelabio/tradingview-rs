use serde::{ Deserialize, Deserializer, Serialize };
pub mod pine_indicator;

#[derive(Debug, Clone, Serialize, Copy, PartialEq)]
pub struct OHLCV {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl OHLCV {
    pub fn new(entry: (f64, f64, f64, f64, f64, f64)) -> Self {
        OHLCV {
            timestamp: (entry.0 * 1000.0) as i64,
            open: entry.1,
            high: entry.2,
            low: entry.3,
            close: entry.4,
            volume: entry.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, Deserialize)]
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
}

#[derive(Debug, Default, Clone, Serialize)]
pub enum SessionType {
    #[default]
    Regular,
    Extended,
    PreMarket,
    PostMarket,
}

impl std::fmt::Display for SessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionType::Regular => write!(f, "regular"),
            SessionType::Extended => write!(f, "extended"),
            SessionType::PreMarket => write!(f, "premarket"),
            SessionType::PostMarket => write!(f, "postmarket"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub enum MarketAdjustment {
    #[default]
    Splits,
    Dividends,
}

impl std::fmt::Display for MarketAdjustment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketAdjustment::Splits => write!(f, "splits"),
            MarketAdjustment::Dividends => write!(f, "dividends"),
        }
    }
}

#[derive(Debug)]
pub enum MarketStatus {
    Holiday,
    Open,
    Close,
    Post,
    Pre,
}

impl std::fmt::Display for MarketStatus {
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

#[derive(Debug, Default)]
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

impl std::fmt::Display for Timezone {
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

#[derive(Default, Clone, Copy, Debug, PartialEq, Serialize)]
pub enum Interval {
    OneSecond,
    FiveSeconds,
    TenSeconds,
    FifteenSeconds,
    ThirtySeconds,
    OneMinute,
    ThreeMinutes,
    FiveMinutes,
    FifteenMinutes,
    ThirtyMinutes,
    FortyFiveMinutes,
    OneHour,
    TwoHours,
    FourHours,
    #[default]
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    SixMonths,
    Yearly,
}

impl std::fmt::Display for Interval {
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
            Interval::Daily => "1D",
            Interval::Weekly => "1W",
            Interval::Monthly => "1M",
            Interval::Quarterly => "3M",
            Interval::SixMonths => "6M",
            Interval::Yearly => "12M",
        };
        write!(f, "{}", time_interval)
    }
}

pub enum LanguageCode {
    Arabic,
    Chinese,
    Czech,
    Danish,
    Catalan,
    Dutch,
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

impl std::fmt::Display for LanguageCode {
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
    FiscalYear, // FY
    FiscalQuarter, // FQ
    FiscalHalfYear, // FH
    TrailingTwelveMonths, // TTM
    UnknownPeriod(String),
}

impl<'de> Deserialize<'de> for FinancialPeriod {
    fn deserialize<D>(deserializer: D) -> Result<FinancialPeriod, D::Error>
        where D: Deserializer<'de>
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

impl std::fmt::Display for FinancialPeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            FinancialPeriod::FiscalYear => write!(f, "FY"),
            FinancialPeriod::FiscalQuarter => write!(f, "FQ"),
            FinancialPeriod::FiscalHalfYear => write!(f, "FH"),
            FinancialPeriod::TrailingTwelveMonths => write!(f, "TTM"),
            FinancialPeriod::UnknownPeriod(ref s) => write!(f, "{}", s),
        }
    }
}

pub enum SymbolType {
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

impl std::fmt::Display for SymbolType {
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

#[derive(Default, Debug, Clone, Copy, Serialize)]
pub enum SymbolMarketType {
    #[default]
    All,
    Stocks,
    Funds,
    Futures,
    Forex,
    Crypto,
    Indices,
    Bonds,
    Economy,
}

impl std::fmt::Display for SymbolMarketType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            SymbolMarketType::All => write!(f, "undefined"),
            SymbolMarketType::Stocks => write!(f, "stocks"),
            SymbolMarketType::Funds => write!(f, "funds"),
            SymbolMarketType::Futures => write!(f, "futures"),
            SymbolMarketType::Forex => write!(f, "forex"),
            SymbolMarketType::Crypto => write!(f, "crypto"),
            SymbolMarketType::Indices => write!(f, "index"),
            SymbolMarketType::Bonds => write!(f, "bond"),
            SymbolMarketType::Economy => write!(f, "economic"),
        }
    }
}
