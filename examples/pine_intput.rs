use std::collections::HashMap;

fn main() {
    let mut inputs: HashMap<String, HashMap<String, String>> = HashMap::new();

    let input_list = vec![
        HashMap::from([
            ("defval", "babaab"),
            ("id", "text"),
            ("isHidden", "true"),
            ("name", "ILScript"),
            ("type", "text"),
        ]),
        HashMap::from([
            ("defval", ""),
            ("id", "pineId"),
            ("isHidden", "true"),
            ("name", "pineId"),
            ("type", "text"),
        ]),
        HashMap::from([
            ("defval", ""),
            ("id", "pineVersion"),
            ("isHidden", "true"),
            ("name", "pineVersion"),
            ("type", "text"),
        ]),
        HashMap::from([
            (
                "defval",
                "{\"indicator\":1,\"plot\":1,\"str\":1,\"math\":1,\"label\":1,\"request.security\":1}",
            ),
            ("id", "pineFeatures"),
            ("isFake", "true"),
            ("isHidden", "true"),
            ("name", "pineFeatures"),
            ("type", "text"),
        ]),
        HashMap::from([
            ("defval", ""),
            ("display", "15"),
            ("id", "in_0"),
            ("isFake", "true"),
            ("name", "Symbol"),
            ("optional", "true"),
            ("type", "symbol"),
        ]),
        HashMap::from([
            ("defval", "Default"),
            ("display", "15"),
            ("id", "in_1"),
            ("isFake", "true"),
            ("name", "Currency"),
            (
                "options",
                "[\"Default\",\"USD\",\"EUR\",\"CAD\",\"JPY\",\"GBP\",\"HKD\",\"CNY\"]",
            ),
            ("type", "text"),
        ]),
        HashMap::from([
            ("defval", "true"),
            ("display", "0"),
            ("group", "Label settings"),
            ("id", "in_2"),
            ("inline", "Change"),
            ("isFake", "true"),
            ("name", "Show Change"),
            ("type", "bool"),
        ]),
        HashMap::from([
            ("defval", "Percentage"),
            ("display", "15"),
            ("group", "Label settings"),
            ("id", "in_3"),
            ("inline", "Change"),
            ("isFake", "true"),
            ("name", ""),
            ("options", "[\"Percentage\",\"Absolute\"]"),
            ("type", "text"),
        ]),
        HashMap::from([
            ("defval", "true"),
            ("display", "0"),
            ("group", "Label settings"),
            ("id", "in_4"),
            ("isFake", "true"),
            ("name", "Show Date"),
            ("type", "bool"),
        ]),
        HashMap::from([
            ("defval", "rgba(0,0,0,0)"),
            ("id", "__chart_fgcolor"),
            ("isFake", "true"),
            ("isHidden", "true"),
            ("name", "chart.fgcolor"),
            ("type", "color"),
        ]),
        HashMap::from([
            ("defval", "rgba(0,0,0,0)"),
            ("id", "__chart_bgcolor"),
            ("isFake", "true"),
            ("isHidden", "true"),
            ("name", "chart.bgcolor"),
            ("type", "color"),
        ]),
        HashMap::from([
            ("defval", ""),
            ("id", "__user_pro_plan"),
            ("isFake", "true"),
            ("isHidden", "true"),
            ("name", ""),
            ("type", "usertype"),
        ]),
    ];

    for input in input_list {
        if ["text", "pineId", "pineVersion"].contains(&input["id"]) {
            continue;
        }

        let inline_name = input["name"]
            .replace(" ", "_")
            .replace(|c: char| !c.is_ascii_alphanumeric(), "");

        let mut input_map: HashMap<String, String> = HashMap::new();
        input_map.insert("name".to_string(), input["name"].to_string());
        input_map.insert(
            "inline".to_string(),
            input
                .get("inline")
                .unwrap_or(&inline_name.as_str())
                .to_string(),
        );
        input_map.insert(
            "internalID".to_string(),
            input
                .get("internalID")
                .unwrap_or(&inline_name.as_str())
                .to_string(),
        );
        input_map.insert(
            "tooltip".to_string(),
            input.get("tooltip").unwrap_or(&"").to_string(),
        );
        input_map.insert("type".to_string(), input["type"].to_string());
        input_map.insert("value".to_string(), input["defval"].to_string());
        input_map.insert(
            "isHidden".to_string(),
            input.get("isHidden").unwrap_or(&"false").to_string(),
        );
        input_map.insert(
            "isFake".to_string(),
            input.get("isFake").unwrap_or(&"false").to_string(),
        );

        if let Some(options) = input.get("options") {
            input_map.insert("options".to_string(), options.to_string());
        }

        inputs.insert(input["id"].to_string(), input_map);
    }

    println!("{:?}", inputs);
}
