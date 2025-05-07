use std::time::Instant;

use chrono::Duration;
use hrdf_parser::Hrdf;
use reqwest::{Error, Response};
use crate::{
    routing::{find_reachable_stops_within_time_limit, plan_journey},
    utils::create_date_time,
};

pub fn run_debug(hrdf: Hrdf) {
    println!();
    println!(
        "------------------------------------------------------------------------------------------------"
    );
    println!("--- Debug");
    println!(
        "------------------------------------------------------------------------------------------------"
    );

    test_plan_journey(&hrdf);
    test_find_reachable_stops_within_time_limit(&hrdf);
}

#[allow(dead_code)]
pub fn test_plan_journey(hrdf: &Hrdf) {
    // ------------------------------------------------------------------------------------------------
    // --- 2.0.5
    // ------------------------------------------------------------------------------------------------

    let verbose = true;

    println!();
    let start_time = Instant::now();

    // 1. Petit-Lancy, Les Esserts => Onex, Bandol
    plan_journey(
        hrdf,
        8587418,
        8593027,
        create_date_time(2025, 6, 1, 12, 30),
        verbose,
    );

    // 2. Petit-Lancy, Les Esserts => Genève-Aéroport
    plan_journey(
        hrdf,
        8587418,
        8501026,
        create_date_time(2025, 2, 9, 14, 2),
        verbose,
    );

    // 3. Avully, village => Pont-Céard, gare
    plan_journey(
        hrdf,
        8587031,
        8593189,
        create_date_time(2025, 7, 13, 16, 43),
        verbose,
    );

    // 4. Petit-Lancy, Les Esserts => Vevey, Palud
    plan_journey(
        hrdf,
        8587418,
        8595120,
        create_date_time(2025, 9, 17, 5, 59),
        verbose,
    );

    // 5. Genève, gare Cornavin => Avusy, village
    plan_journey(
        hrdf,
        8587057,
        8587032,
        create_date_time(2025, 10, 18, 20, 10),
        verbose,
    );

    // 6. Genève => Bern, Bierhübeli
    plan_journey(
        hrdf,
        8501008,
        8590028,
        create_date_time(2025, 11, 22, 6, 59),
        verbose,
    );

    // 7. Genève => Zürich HB
    plan_journey(
        hrdf,
        8501008,
        8503000,
        create_date_time(2025, 4, 9, 8, 4),
        verbose,
    );

    // 8. Zürich HB => Lugano, Genzana
    plan_journey(
        hrdf,
        8503000,
        8575310,
        create_date_time(2025, 6, 15, 12, 10),
        verbose,
    );

    // 9. Chancy, Douane => Campocologno
    plan_journey(
        hrdf,
        8587477,
        8509368,
        create_date_time(2025, 5, 29, 17, 29),
        verbose,
    );

    // 10. Chancy, Douane => Sevelen, Post
    plan_journey(
        hrdf,
        8587477,
        8581989,
        create_date_time(2025, 9, 10, 13, 37),
        true,
    );

    //11. Genève => Paris gare de Lyon
    plan_journey(
        hrdf,
        8501008,
        8768600,
        create_date_time(2025, 4, 28, 8, 29),
        true,
    );

    //12. Genève => Lausanne
    plan_journey(
        hrdf,
        8501008,
        8501120,
        create_date_time(2025, 4, 28, 8, 20),
        true,
    );

    println!("\n{:.2?}", start_time.elapsed());
}

#[allow(dead_code)]
pub fn test_find_reachable_stops_within_time_limit(hrdf: &Hrdf) {
    // 1. Petit-Lancy, Les Esserts (8587418)
    let departure_stop_id = 8587418;
    let departure_at = create_date_time(2025, 6, 1, 12, 30);

    // 2. Sevelen, Post (8588197)
    // let departure_stop_id = 8588197;
    // let departure_at = create_date_time(2025, 9, 2, 14, 2);

    // 3. Avully, village (8587031)
    // let departure_stop_id = 8587031;
    // let departure_at = create_date_time(2025, 7, 13, 16, 43);

    // 4. Bern, Bierhübeli (8590028)
    // let departure_stop_id = 8590028;
    // let departure_at = create_date_time(2025, 9, 17, 5, 59);

    // 5. Genève, gare Cornavin (8587057)
    // let departure_stop_id = 8587057;
    // let departure_at = create_date_time(2025, 10, 18, 20, 10);

    // 6. Villmergen, Zentrum (8587554)
    // let departure_stop_id = 8587554;
    // let departure_at = create_date_time(2025, 11, 22, 6, 59);

    // 7. Lugano, Genzana (8575310)
    // let departure_stop_id = 8575310;
    // let departure_at = create_date_time(2025, 4, 9, 8, 4);

    // 8. Zürich HB (8503000)
    // let departure_stop_id = 8503000;
    // let departure_at = create_date_time(2025, 6, 15, 12, 10);

    // 9. Campocologno (8509368)
    // let departure_stop_id = 8509368;
    // let departure_at = create_date_time(2025, 5, 29, 17, 29);

    // 10. Chancy, Douane (8587477)
    // let departure_stop_id = 8587477;
    // let departure_at = create_date_time(2025, 9, 10, 13, 37);

    let start_time = Instant::now();
    for time_limit in [60, 120, 180] {
        let routes = find_reachable_stops_within_time_limit(
            hrdf,
            departure_stop_id,
            departure_at,
            Duration::minutes(time_limit),
            false,
        );

        println!("\n{}", routes.len());
    }

    println!("{:.2?}", start_time.elapsed());
}



#[cfg(test)]
mod tests {
    use std::time::Duration;
    use chrono::{DateTime, Local, NaiveDate, Utc};
    use crate::debug::{test_find_reachable_stops_within_time_limit, test_plan_journey};

    use hrdf_parser::{Hrdf, Version};
    use serde::{Deserialize, Serialize};
    use test_log::test;

    #[test(tokio::test)]
    async fn debug() {
        let hrdf = Hrdf::new(
            Version::V_5_40_41_2_0_7,
            "https://data.opentransportdata.swiss/en/dataset/timetable-54-2025-hrdf/permalink",
            false,
            None,
        )
        .await
        .unwrap();

        test_plan_journey(&hrdf);
        test_find_reachable_stops_within_time_limit(&hrdf);
    }

    // request structure
    #[derive(Serialize, Deserialize)]
    struct OJP{
        ojp_req: OJPReq
    }
    #[derive(Serialize, Deserialize)]
    struct OJPReq{
        service_req: ServiceReq
    }
    #[derive(Serialize, Deserialize)]
    struct ServiceReq{
        req_timestamp: DateTime<Local>,
        req_ref: String,
        ojp_trip_req: OJPTripReq,
    }
    #[derive(Serialize, Deserialize)]
    struct OJPTripReq{
        req_timestamp: DateTime<Local>,
        msg_id: String,
        origin: PlaceRef,
        destination: PlaceRef,
        params: ReqParams
    }
    #[derive(Serialize, Deserialize)]
    struct PlaceRef{
        stop_pt_ref: i32,
        name: String,
    }
    #[derive(Serialize, Deserialize)]
    struct ReqParams {
        mode_and_mode_of_op_filter: ModeFilter,
        result_nb: i32,
    }
    #[derive(Serialize, Deserialize)]
    struct ModeFilter{
        exclude: bool,
        pt_mode: String,
    }

    // response structures
    #[derive(Serialize, Deserialize)]
    struct OJPResp{
        siri: ServiceDelivery
    }
    #[derive(Serialize, Deserialize)]
    struct ServiceDelivery{
        responde_timestamp: DateTime<Local>,
        producer_ref: String,
        ojp_trip_delivery: OJPTripDelivery
    }
    #[derive(Serialize, Deserialize)]
    struct OJPTripDelivery{
        responde_timestamp: DateTime<Local>,
        request_msg_ref: String,
        default_lang: String,
        trip_response_ctx: Vec<Place>,
        trip_result: Vec<TripResult>,
    }
    #[derive(Serialize, Deserialize)]
    struct Place{
        place: PlaceType,
        name: String,
        geo_position: GeoPosition
    }
    #[derive(Serialize, Deserialize)]
    enum PlaceType{
        StopPlace{stop_place: StopPlace},
        StopPoint{stop_point: StopPoint},
        TopographicPlace{topographic_place: TopographicPlace}
    }
    #[derive(Serialize, Deserialize)]
    struct StopPoint{
        stop_point_ref: String,
        name: String,
        private_code: PrivateCode,
        parent_ref: i32,
        topographic_place_ref: String,
    }
    #[derive(Serialize, Deserialize)]
    struct TopographicPlace{
        topographic_place_code: String,
        name: String,
    }
    #[derive(Serialize, Deserialize)]
    struct StopPlace{
        stop_place_ref: i32,
        name: String,
        private_code: PrivateCode,
        topographic_place_ref: String,
    }
    #[derive(Serialize, Deserialize)]
    struct PrivateCode {
        system: String,
        value: String,
    }

    #[derive(Serialize, Deserialize)]
    struct GeoPosition{
        longitude: f64,
        latitude: f64,
    }

    #[derive(Serialize, Deserialize)]
    struct TripResult{
        trip_id: String,
        trip: Trip,
    }
    #[derive(Serialize, Deserialize)]
    struct Trip{
        trip_id: String,
        duration: Duration,
        start_time: DateTime<Local>,
        end_time: DateTime<Local>,
        transfers: i32,
        distance: i32,
        legs: Vec<Leg>
    }
    #[derive(Serialize, Deserialize)]
    struct Leg{
        id: i32,
        duration: Duration,
        timed_leg: TimedLeg,
        emission_co2: f64,
    }
    #[derive(Serialize, Deserialize)]
    struct TimedLeg{
        leg_board: LegBoard,
        leg_align: LegAlign,
        service: Service,
    }
    #[derive(Serialize, Deserialize)]
    struct LegBoard{
        stop_point_ref: String,
        stop_point_name: String,
        name_suffix: String,
        planned_quay: i32,
        estimated_quay: i32,
        service_departure: ServiceTime,
        order: i32
    }
    #[derive(Serialize, Deserialize)]
    struct LegAlign{
        stop_point_ref: String,
        stop_point_name: String,
        name_suffix: String,
        planned_quay: i32,
        estimated_quay: i32,
        service_arrival: ServiceTime,
        order: i32
    }
    #[derive(Serialize, Deserialize)]
    struct Service{
        operating_day_ref: NaiveDate,
        journey_ref: String,
        public_code: String,
        line_ref: String,
        direction_ref: String,
        mode: Mode,
        product_category: ProductCategory,
        published_service_name: PublishedServiceName,
        attributes: Vec<Attribute>,
        origin_text: String,
        operation_ref: i32,
        destination_stop_point_ref: i32,
        destination_text: String,
    }

    #[derive(Serialize, Deserialize)]
    struct Mode{
        pt_mode: String,
        rail_submodule: String, // todo check if always there
        name: String,
        short_name: String,
    }
    #[derive(Serialize, Deserialize)]
    struct ProductCategory{
        name: String,
        short_name: String,
        product_category_ref: i32,
    }
    #[derive(Serialize, Deserialize)]
    struct PublishedServiceName{text: String}
    #[derive(Serialize, Deserialize)]
    struct Attribute{
        user_text: String,
        code: String,
    }

    #[derive(Serialize, Deserialize)]
    struct ServiceTime {
        timetable_time: DateTime<Local>,
        estimated_time: DateTime<Local>,
    }

    #[test(tokio::test)]
    pub async fn test_paths_validity(){
        // First build hrdf file
        let hrdf = Hrdf::new(
            Version::V_5_40_41_2_0_7,
            "https://data.opentransportdata.swiss/en/dataset/timetable-54-2025-hrdf/permalink",
            false,
            None,
        )
            .await
            .unwrap();
        
        
        let client = reqwest::Client::new();
        // reqwest et serde pour récupérer les infos
        let content = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
                            <OJP xmlns=\"http://www.vdv.de/ojp\" xmlns:siri=\"http://www.siri.org.uk/siri\" version=\"2.0\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:schemaLocation=\"http://www.vdv.de/ojp ../../../../Downloads/OJP-changes_for_v1.1%20(1)/OJP-changes_for_v1.1/OJP.xsd\">
                             <OJPRequest>
                             <siri:ServiceRequest>
                             <siri:RequestTimestamp>2024-06-01T11:16:59.475Z</siri:RequestTimestamp>
                             <siri:RequestorRef>MENTZRegTest</siri:RequestorRef>
                             <OJPLocationInformationRequest>
                             <siri:RequestTimestamp>2024-06-01T11:16:59.475Z</siri:RequestTimestamp>
                             <siri:MessageIdentifier>LIR-1a</siri:MessageIdentifier>
                             <InitialInput>
                             <Name>Bern</Name>
                             </InitialInput>
                             <Restrictions>
                             <Type>stop</Type>
                             <NumberOfResults>10</NumberOfResults>
                             </Restrictions>
                             </OJPLocationInformationRequest>
                             </siri:ServiceRequest>
                             </OJPRequest>
                             </OJP>";
        let res = client.post("http://httpbin.org/post")
            .header("Authorization","Bearer eyJvcmciOiI2NDA2NTFhNTIyZmEwNTAwMDEyOWJiZTEiLCJpZCI6IjQ2NWE4N2MwOThkMzRlMzFiN2I5YmRmMDg1MGFjZWQxIiwiaCI6Im11cm11cjEyOCJ9")
            .body(content)
            .send()
            .await;

        // Do the path search
        match res{
            Ok(res) => {println!("{:?}", res)}
            _ => {}
        }
        // compare path duration
    }
}
