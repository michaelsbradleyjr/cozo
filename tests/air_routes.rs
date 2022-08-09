use std::fs::read_to_string;
use std::str::FromStr;
use std::time::Instant;

use anyhow::Result;
use serde_json::json;

use cozo::Db;
use cozorocks::DbBuilder;

fn create_db(name: &str, destroy_on_exit: bool) -> Db {
    let builder = DbBuilder::default()
        .path(name)
        .create_if_missing(true)
        .destroy_on_exit(destroy_on_exit);
    Db::build(builder).unwrap()
}

fn init_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

#[test]
fn air_routes() -> Result<()> {
    init_logger();
    let db = create_db("_test_air_routes", false);
    let attr_res = db.run_tx_attributes(
        r#"
        put country {
            code: string identity,
            desc: string
        }
        put continent {
            code: string identity,
            desc: string
        }
        put airport {
            iata: string identity,
            icao: string index,
            city: string index,
            desc: string,
            region: string index,
            country: ref,
            runways: int,
            longest: int,
            altitude: int,
            lat: float,
            lon: float
        }
        put route {
            src: ref,
            dst: ref,
            distance: int
        }
        put geo {
            contains: ref
        }
    "#,
    );

    if attr_res.is_ok() {
        let insertions = read_to_string("tests/air-routes-data.json")?;
        let triple_insertion_time = Instant::now();
        db.run_tx_triples(&insertions)?;
        dbg!(triple_insertion_time.elapsed());
    }

    let simple_query_time = Instant::now();
    let res = db.run_script(r#"
        ?[?c, ?code, ?desc] := [?c country.code 'CU'] or ?c is 10000239, [?c country.code ?code], [?c country.desc ?desc];
    "#)?;
    dbg!(simple_query_time.elapsed());
    assert_eq!(
        res,
        json!([[10000060, "CU", "Cuba"], [10000239, "VN", "Viet Nam"]])
    );

    let no_airports_time = Instant::now();
    let res = db.run_script(
        r#"
        ?[?desc] := [?c country.desc ?desc], not [?a airport.country ?c];
    "#,
    )?;
    dbg!(no_airports_time.elapsed());
    assert_eq!(
        res,
        json!([
            ["Andorra"],
            ["Liechtenstein"],
            ["Monaco"],
            ["Pitcairn"],
            ["San Marino"]
        ])
    );

    let no_routes_airport_time = Instant::now();
    let res = db.run_script(
        r#"
        ?[?code] := [?a airport.iata ?code], not [?_ route.src ?a], not [?_ route.dst ?a];
    "#,
    )?;
    dbg!(no_routes_airport_time.elapsed());
    assert_eq!(
        res,
        serde_json::Value::from_str(
            r#"[
            ["AFW"],["APA"],["APK"],["BID"],["BVS"],["BWU"],["CRC"],["CVT"],["EKA"],["GYZ"],
            ["HFN"],["HZK"],["ILG"],["INT"],["ISL"],["KGG"],["NBW"],["NFO"],["PSY"],["RIG"],
            ["SFD"],["SFH"],["SXF"],["TUA"],["TWB"],["TXL"],["VCV"],["YEI"]
        ]"#
        )
        .unwrap()
    );

    let runway_distribution_time = Instant::now();
    let res = db.run_script(
        r#"
        ?[?runways, count(?a)] := [?a airport.runways ?runways];
    "#,
    )?;
    dbg!(runway_distribution_time.elapsed());
    assert_eq!(
        res,
        json!([
            [1, 2429],
            [2, 775],
            [3, 227],
            [4, 53],
            [5, 14],
            [6, 4],
            [7, 2]
        ])
    );

    let most_out_routes_time = Instant::now();
    let res = db.run_script(
        r#"
        route_count[?a, count(?r)] := [?r route.src ?a];
        ?[?code, ?n] := route_count[?a, ?n], ?n > 180, [?a airport.iata ?code];
        :sort -?n;
    "#,
    )?;
    dbg!(most_out_routes_time.elapsed());
    assert_eq!(
        res,
        serde_json::Value::from_str(
            r#"[
        ["FRA",307],["IST",307],["CDG",293],["AMS",282],["MUC",270],["ORD",264],["DFW",251],
        ["PEK",248],["DXB",247],["ATL",242],["DME",232],["LGW",232],["LHR",221],["DEN",216],
        ["MAN",216],["LAX",213],["PVG",212],["STN",211],["MAD",206],["VIE",206],["BCN",203],
        ["BER",202],["FCO",201],["JFK",201],["DUS",199],["IAH",199],["EWR",197],["MIA",195],
        ["YYZ",195],["BRU",194],["CPH",194],["DOH",186],["DUB",185],["CLT",184],["SVO",181]
        ]"#
        )
        .unwrap()
    );

    let most_routes_time = Instant::now();
    let res = db.run_script(
        r#"
        route_count[?a, count(?r)] := [?r route.src ?a] or [?r route.dst ?a];
        ?[?code, ?n] := route_count[?a, ?n], ?n > 400, [?a airport.iata ?code];
        :sort -?n;
    "#,
    )?;
    dbg!(most_routes_time.elapsed());
    assert_eq!(
        res,
        serde_json::Value::from_str(
            r#"[
        ["FRA",614],["IST",614],["CDG",587],["AMS",566],["MUC",541],["ORD",527],["DFW",502],
        ["PEK",497],["DXB",494],["ATL",484],["DME",465],["LGW",464],["LHR",442],["DEN",432],
        ["MAN",431],["LAX",426],["PVG",424],["STN",423],["MAD",412],["VIE",412],["BCN",406],
        ["BER",404],["FCO",402],["JFK",401]]"#
        )
        .unwrap()
    );

    let airport_with_one_route_time = Instant::now();
    let res = db.run_script(
        r#"
        route_count[?a, count(?r)] := [?r route.src ?a];
        ?[count(?a)] := route_count[?a, ?n], ?n == 1;
    "#,
    )?;
    dbg!(airport_with_one_route_time.elapsed());
    assert_eq!(res, json!([[777]]));

    let single_runway_with_most_routes_time = Instant::now();
    let res = db.run_script(r#"
        single_or_lgw[?a] := [?a airport.iata 'LGW'] or [?a airport.runways 1];
        out_counts[?a, count(?r)] := single_or_lgw[?a], [?r route.src ?a];
        ?[?code, ?city, ?out_n] := out_counts[?a, ?out_n], [?a airport.city ?city], [?a airport.iata ?code];

        :order -?out_n;
        :limit 10;
    "#)?;
    dbg!(single_runway_with_most_routes_time.elapsed());
    assert_eq!(
        res,
        serde_json::Value::from_str(
            r#"[
        ["LGW","London",232],["STN","London",211],["CTU","Chengdu",139],["LIS","Lisbon",139],
        ["BHX","Birmingham",130],["LTN","London",130],["SZX","Shenzhen",129],
        ["CKG","Chongqing",122],["STR","Stuttgart",121],["CRL","Brussels",117]]"#
        )
        .unwrap()
    );

    let most_routes_in_canada_time = Instant::now();
    let res = db.run_script(r#"
        ca_airports[?a, count(?r)] := [?c country.code 'CA'], [?a airport.country ?c], [?r route.src ?a];
        ?[?code, ?city, ?n_routes] := ca_airports[?a, ?n_routes], [?a airport.iata ?code], [?a airport.city ?city];

        :order -?n_routes;
        :limit 10;
    "#)?;
    dbg!(most_routes_in_canada_time.elapsed());
    assert_eq!(
        res,
        json!([
            ["YYZ", "Toronto", 195],
            ["YUL", "Montreal", 121],
            ["YVR", "Vancouver", 105],
            ["YYC", "Calgary", 74],
            ["YEG", "Edmonton", 47],
            ["YHZ", "Halifax", 45],
            ["YWG", "Winnipeg", 38],
            ["YOW", "Ottawa", 36],
            ["YZF", "Yellowknife", 21],
            ["YQB", "Quebec City", 20]
        ])
    );

    let uk_count_time = Instant::now();
    let res = db.run_script(r"
        ?[?region, count(?a)] := [?c country.code 'UK'], [?a airport.country ?c], [?a airport.region ?region];
    ")?;
    dbg!(uk_count_time.elapsed());
    assert_eq!(
        res,
        json!([["GB-ENG", 27], ["GB-NIR", 3], ["GB-SCT", 25], ["GB-WLS", 3]])
    );

    let airports_by_country = Instant::now();
    let res = db.run_script(
        r"
        airports_by_country[?c, count(?a)] := [?a airport.country ?c];
        country_count[?c, max(?count)] := airports_by_country[?c, ?count];
        ?[?code, ?count] := [?c country.code ?code], country_count[?c, ?count];
        ?[?code, ?count] := [?c country.code ?code], not country_count[?c, ?_], ?count is 0;

        :order ?count;
    ",
    )?;
    dbg!(airports_by_country.elapsed());
    assert_eq!(
        res,
        serde_json::Value::from_str(
            r#"[
    ["AD",0],["LI",0],["MC",0],["PN",0],["SM",0],["AG",1],["AI",1],["AL",1],["AS",1],["AW",1],
    ["BB",1],["BH",1],["BI",1],["BJ",1],["BL",1],["BM",1],["BN",1],["BT",1],["CC",1],["CF",1],
    ["CW",1],["CX",1],["DJ",1],["DM",1],["ER",1],["FO",1],["GD",1],["GF",1],["GI",1],["GM",1],
    ["GN",1],["GP",1],["GU",1],["GW",1],["HK",1],["IM",1],["JE",1],["KM",1],["KP",1],["KS",1],
    ["KW",1],["LB",1],["LS",1],["LU",1],["LV",1],["MD",1],["MF",1],["ML",1],["MO",1],["MQ",1],
    ["MS",1],["MT",1],["NC",1],["NE",1],["NF",1],["NI",1],["NR",1],["PM",1],["PW",1],["QA",1],
    ["SL",1],["SR",1],["SS",1],["ST",1],["SV",1],["SX",1],["SZ",1],["TG",1],["TL",1],["TM",1],
    ["TV",1],["VC",1],["WS",1],["YT",1],["AM",2],["BF",2],["CI",2],["EH",2],["FK",2],["GA",2],
    ["GG",2],["GQ",2],["GT",2],["GY",2],["HT",2],["HU",2],["JM",2],["JO",2],["KG",2],["KI",2],
    ["KN",2],["LC",2],["LR",2],["ME",2],["MH",2],["MK",2],["MP",2],["MU",2],["PY",2],["RE",2],
    ["RW",2],["SC",2],["SG",2],["SH",2],["SI",2],["SK",2],["SY",2],["TT",2],["UY",2],["VG",2],
    ["VI",2],["WF",2],["BQ",3],["BY",3],["CG",3],["CY",3],["EE",3],["GE",3],["KH",3],["KY",3],
    ["LT",3],["MR",3],["RS",3],["ZW",3],["BA",4],["BG",4],["BW",4],["FM",4],["OM",4],["SN",4],
    ["TC",4],["TJ",4],["UG",4],["AF",5],["AZ",5],["BE",5],["CM",5],["CZ",5],["NL",5],["PA",5],
    ["SD",5],["TD",5],["TO",5],["AT",6],["CH",6],["CK",6],["GH",6],["HN",6],["IL",6],["IQ",6],
    ["LK",6],["SO",6],["BD",7],["CV",7],["DO",7],["IE",7],["IS",7],["MW",7],["PR",7],["DK",8],
    ["HR",8],["LA",8],["MV",8],["TN",8],["TW",9],["YE",9],["ZM",9],["AE",10],["FJ",10],["MN",10],
    ["CD",11],["EG",11],["LY",11],["MZ",11],["NP",11],["TZ",11],["UZ",11],["CU",12],["BZ",13],
    ["CR",13],["MG",13],["PL",13],["AO",14],["GL",14],["KE",14],["RO",14],["BO",15],["EC",15],
    ["KR",15],["UA",15],["ET",16],["MA",16],["CL",17],["MM",17],["SB",17],["BS",18],["NG",19],
    ["PT",19],["FI",20],["ZA",20],["KZ",21],["PK",21],["PE",22],["VN",22],["NZ",25],["PG",26],
    ["SA",26],["VU",26],["VE",27],["DZ",30],["TH",33],["DE",34],["MY",35],["AR",38],["IT",38],
    ["GR",39],["PF",39],["SE",39],["PH",40],["ES",43],["IR",45],["NO",49],["CO",51],["TR",52],
    ["UK",58],["FR",59],["MX",60],["JP",65],["ID",70],["IN",77],["BR",117],["RU",129],["AU",132],
    ["CA",205],["CN",217],["US",586]]"#
        )
        .unwrap()
    );

    let n_airports_by_continent_time = Instant::now();
    let res = db.run_script(
        r#"
        airports_by_continent[?c, count(?a)] := [?a airport.iata ?_], [?c geo.contains ?a];
        ?[?cont, max(?count)] := airports_by_continent[?c, ?count], [?c continent.code ?cont];
        ?[?cont, max(?count)] := [?_ continent.code ?cont], ?count is 0;
    "#,
    )?;
    dbg!(n_airports_by_continent_time.elapsed());
    assert_eq!(
        res,
        serde_json::Value::from_str(
            r#"[["AF",321],["AN",0],["AS",971],["EU",605],["NA",989],["OC",305],["SA",313]]"#
        )
        .unwrap()
    );

    let routes_per_airport_time = Instant::now();
    let res = db.run_script(
        r#"
        routes_count[?a, count(?r)] := given[?code], [?a airport.iata ?code], [?r route.src ?a];
        ?[?code, ?n] := routes_count[?a, ?n], [?a airport.iata ?code];

        given <- [['A' ++ 'U' ++ 'S'],['AMS'],['JFK'],['DUB'],['MEX']];
        "#,
    )?;
    dbg!(routes_per_airport_time.elapsed());
    assert_eq!(
        res,
        serde_json::Value::from_str(
            r#"[["AMS",282],["AUS",95],["DUB",185],["JFK",201],["MEX",116]]"#
        )
        .unwrap()
    );

    let airports_by_route_number_time = Instant::now();
    let res = db.run_script(
        r#"
        route_count[?a, count(?r)] := [?r route.src ?a];
        ?[?n, collect(?code)] := route_count[?a, ?n], [?a airport.iata ?code], ?n = 105;
    "#,
    )?;
    dbg!(airports_by_route_number_time.elapsed());
    assert_eq!(res, json!([[105, ["TFS", "YVR"]]]));

    let out_from_aus_time = Instant::now();
    let res = db.run_script(r#"
        out_by_runways[?n_runways, count(?a)] := [?aus airport.iata 'AUS'],
                                                 [?r1 route.src ?aus],
                                                 [?r1 route.dst ?a],
                                                 [?a airport.runways ?n_runways];
        two_hops[count(?a)] := [?aus airport.iata 'AUS'],
                               [?r1 route.src ?aus],
                               [?r1 route.dst ?a],
                               [?r route.src ?a];
        ?[max(?total), collect(?coll)] := two_hops[?total], out_by_runways[?n, ?ct], ?coll is [?n, ?ct];
    "#)?;
    dbg!(out_from_aus_time.elapsed());
    assert_eq!(
        res,
        serde_json::Value::from_str(r#"[[7909,[[1,9],[2,23],[3,29],[4,24],[5,5],[6,3],[7,2]]]]"#)
            .unwrap()
    );

    let const_return_time = Instant::now();
    let res = db.run_script(
        r#"
        ?[?name, count(?a)] := [?a airport.region 'US-OK'], ?name is 'OK';
    "#,
    )?;
    dbg!(const_return_time.elapsed());
    assert_eq!(res, json!([["OK", 4]]));

    let multi_res_time = Instant::now();
    let res = db.run_script(r#"
        total[count(?a)] := [?a airport.iata ?_];
        high[count(?a)] := [?a airport.runways ?n], ?n >= 6;
        low[count(?a)] := [?a airport.runways ?n], ?n <= 2;
        four[count(?a)] := [?a airport.runways ?n], ?n = 4;
        france[count(?a)] := [?fr country.code 'FR'], [?a airport.country ?fr];

        ?[?total, ?high, ?low, ?four, ?france] := total[?total], high[?high], low[?low],
                                                  four[?four], france[?france];
    "#)?;
    dbg!(multi_res_time.elapsed());
    assert_eq!(
        res,
        serde_json::Value::from_str(r#"[[3504,6,3204,53,59]]"#).unwrap()
    );

    Ok(())
}