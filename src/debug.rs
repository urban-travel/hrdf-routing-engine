use std::time::Instant;

use chrono::Duration;
use hrdf_parser::Hrdf;

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
    use std::cmp::Ordering;
    use chrono::{DateTime, Local, NaiveDate};
    use crate::debug::{test_find_reachable_stops_within_time_limit, test_plan_journey};

    use hrdf_parser::{Hrdf, Version};
    use serde::{Deserialize, Serialize};
    use test_log::test;
    use xml::{EventReader, ParserConfig};
    use serde_xml_rs::{de::Deserializer};
    use crate::routing::plan_journey;
    use crate::utils::create_date_time;

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
    struct OJPResponse{
        #[serde(rename="OJPResponse")]
        ojp_response: OJPResp
    }
    #[derive(Serialize, Deserialize)]
    struct OJPResp{
        #[serde(rename="siri:ServiceDelivery")]
        siri_service_delivery: ServiceDelivery
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct ServiceDelivery{
        #[serde(rename="siri:ResponseTimestamp")]
        response_timestamp: DateTime<Local>,
        #[serde(rename="siri:ProducerRef")]
        producer_ref: String,
        o_j_p_trip_delivery: OJPTripDelivery
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct OJPTripDelivery{
        #[serde(rename="siri:ResponseTimestamp")]
        response_timestamp: DateTime<Local>,
        #[serde(rename="siri:RequestMessageRef")]
        request_msg_ref: String,
        #[serde(rename="siri:DefaultLanguage")]
        default_lang: String,
        calc_time: i32,
        trip_response_context: TripResponseContext,
        trip_result: Vec<TripResult>,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct TripResponseContext{
        places: Place,
        situations: Situations,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Place{
        place: Vec<PlaceCnt>
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Situations{
        pt_situation: Vec<PtSituation>
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct PtSituation{
        #[serde(rename="siri:CreationTime")]
        creation_time: DateTime<Local>,
        #[serde(rename="siri:ParticipantRef")]
        participant_ref: String,
        #[serde(rename="siri:SituationNumber")]
        situation_number: String,
        #[serde(rename="siri:Version")]
        version: i32,
        #[serde(rename="siri:Source")]
        source: SourceType,
        #[serde(rename="siri:ValidityPeriod")]
        validity_period: Vec<TimePeriod>,
        #[serde(rename="siri:AlertCause")]
        alert_cause: String,
        #[serde(rename="siri:Priority")]
        priority: String,
        #[serde(rename="siri:ScopeType")]
        scope_type: String,
        #[serde(rename="siri:Language")]
        language: String,
        #[serde(rename="siri:PublishingActions")]
        publishing_actions: PublishingActions,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct SourceType{
        #[serde(rename="siri:SourceType")]
        source_type: String,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct TimePeriod{
        #[serde(rename="siri:StartTime")]
        start_time: DateTime<Local>,
        #[serde(rename="siri:EndTime")]
        end_time: DateTime<Local>,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct PublishingActions{
        #[serde(rename="siri:PublishingAction")]
        publishing_action: Vec<PublishingAction>,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct PublishingAction{
        #[serde(rename="siri:PublishAtScope")]
        publishing_at_scope: PublishAtScope,
        #[serde(rename="siri:PassengerInformationAction")]
        passenger_information_action: PassengerInformationAction,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct PublishAtScope{
        #[serde(rename="siri:ScopeType")]
        scope_type: String,
        #[serde(rename="siri:Affects")]
        affects: (),
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct PassengerInformationAction{
        #[serde(rename="siri:ActionRef")]
        action_ref: (),
        #[serde(rename="siri:RecordedAtTime")]
        record_at_time: DateTime<Local>,
        #[serde(rename="siri:Perspective")]
        perspective: String,
        #[serde(rename="siri:TextualContent")]
        recommendation_content: TextualContent,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct TextualContent{
        #[serde(rename="siri:SummaryContent")]
        summary_content: SummaryContent,
        #[serde(rename="siri:ReasonContent")]
        reason_content: ReasonContent,
        #[serde(rename="siri:RecommendationContent")]
        recommendation_content: RecommendationContent,
        #[serde(rename="siri:DurationContent")]
        duration_content: DurationContent,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct SummaryContent{
        #[serde(rename="siri:SummaryText")]
        summary_text: String,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct RecommendationContent{
        #[serde(rename="siri:RecommendationText")]
        recommendation_text: String,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct DurationContent{
        #[serde(rename="siri:DurationText")]
        duration_text: String,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct ReasonContent{
        #[serde(rename="siri:ReasonText")]
        reason_text: String,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct PlaceCnt {
        #[serde(rename = "#content")]
        place: PlaceType,
        name: Text,
        geo_position: GeoPosition
    }
    #[derive(Serialize, Deserialize)]
    enum PlaceType{
        #[serde(rename_all = "PascalCase")]
        StopPlace{
            stop_place_ref: i32,
            stop_place_name: Text,
            private_code: PrivateCode,
            topographic_place_ref: String,
        },
        #[serde(rename_all = "PascalCase")]
        StopPoint{
            #[serde(rename="siri:StopPointRef")]
            stop_point_ref: String,
            stop_point_name: Text,
            private_code: PrivateCode,
            parent_ref: i32,
            topographic_place_ref: String,
        },
        #[serde(rename_all = "PascalCase")]
        TopographicPlace{
            topographic_place_code: String,
            topographic_place_name: Text,
        }
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct PrivateCode {
        system: String,
        value: String,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct GeoPosition{
        #[serde(rename="siri:Longitude")]
        longitude: f64,
        #[serde(rename="siri:Latitude")]
        latitude: f64,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct TripResult{
        id: String,
        trip: Trip,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Trip{
        id: String,
        duration: String, //Duration,
        start_time: DateTime<Local>, // todo use this to compute duration
        end_time: DateTime<Local>,
        transfers: i32,
        distance: i32,
        leg: Vec<Leg>
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Leg{
        id: i32,
        duration: String, //Duration,
        #[serde(rename = "#content")]
        timed_leg: TypeLeg,
        emission_c_o2: EmissionCO2,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct EmissionCO2{
        kilogram_per_person_km: f64
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    enum TypeLeg{
        #[serde(rename_all = "PascalCase")]
        TimedLeg{
            leg_board: LegBoard,
            leg_alight: LegAlight,
            service: Service,
        },
        #[serde(rename_all = "PascalCase")]
        TransferLeg{
            transfer_type: String,
            leg_start: LegPoint,
            leg_end: LegPoint,
            duration: String,
        }
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct LegPoint{
        #[serde(rename="siri:StopPointRef")]
        stop_point_ref: String,
        name: Text,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct LegBoard{
        #[serde(rename="siri:StopPointRef")]
        stop_point_ref: String,
        stop_point_name: Text,
        name_suffix: Text,
        planned_quay: Text,
        estimated_quay: Option<Text>,
        service_departure: ServiceTime,
        order: i32
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct LegAlight {
        #[serde(rename="siri:StopPointRef")]
        stop_point_ref: String,
        stop_point_name: Text,
        name_suffix: Text,
        planned_quay: Text,
        estimated_quay: Option<Text>,
        service_arrival: ServiceTime,
        order: i32
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Service{
        operating_day_ref: NaiveDate,
        journey_ref: String,
        public_code: String,
        #[serde(rename="siri:LineRef")]
        line_ref: String,
        #[serde(rename="siri:DirectionRef")]
        direction_ref: String,
        mode: Mode,
        product_category: ProductCategory,
        published_service_name: Text,
        train_number: i32,
        attribute: Vec<Attribute>,
        origin_text: Text,
        #[serde(rename="siri:OperatorRef")]
        operation_ref: i32,
        destination_stop_point_ref: i32,
        destination_text: Text,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Mode{
        pt_mode: String,
        #[serde(rename="siri:RailSubmode")]
        rail_submodule: String, // todo check if always there
        name: Text,
        short_name: Text,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct ProductCategory{
        name: Text,
        short_name: Text,
        product_category_ref: i32,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Attribute{
        user_text: Text,
        code: String,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct ServiceTime {
        timetabled_time: DateTime<Local>,
        estimated_time: Option<DateTime<Local>>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Text{
        text: String,
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
        
        let from_point_ref = 8501008; // Geneva
        let to_point_ref = 8501120; // Lausanne
        let expected_result_nb = 3;
        let journey_year = 2025;
        let journey_month = 9;
        let journey_day = 1;
        let journey_hour = 11;
        let journey_minute = 16;
        let journey_second = 17;
        
        let client = reqwest::Client::new();
        // reqwest et serde pour récupérer les infos
        let content = format!{"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
                            <OJP xmlns=\"http://www.vdv.de/ojp\" xmlns:siri=\"http://www.siri.org.uk/siri\" version=\"2.0\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:schemaLocation=\"http://www.vdv.de/ojp ../../../../Downloads/OJP-changes_for_v1.1%20(1)/OJP-changes_for_v1.1/OJP.xsd\">
                             	<OJPRequest>
                                    <siri:ServiceRequest>
                                        <siri:RequestTimestamp>{journey_year}-{journey_month:0>2}-{journey_day:0>2}T{journey_hour:0>2}:{journey_minute:0>2}:{journey_second:0>2}.475Z</siri:RequestTimestamp>
                                        <siri:RequestorRef>Hepisochrone</siri:RequestorRef>
                                        <OJPTripRequest>
                                            <siri:RequestTimestamp>{journey_year}-{journey_month:0>2}-{journey_day:0>2}T{journey_hour:0>2}:{journey_minute:0>2}:{journey_second:0>2}.475Z</siri:RequestTimestamp>
                                            <siri:MessageIdentifier>TR-1h2</siri:MessageIdentifier>
                                            <Origin>
                                                <PlaceRef>
                                                    <siri:StopPointRef>{from_point_ref}</siri:StopPointRef>
                                                </PlaceRef>
                                                <DepArrTime>{journey_year}-{journey_month:0>2}-{journey_day:0>2}T{journey_hour:0>2}:{journey_minute:0>2}:{journey_second:0>2}.475Z</DepArrTime>
                                            </Origin>
                                            <Destination>
                                                <PlaceRef>
                                                    <siri:StopPointRef>{to_point_ref}</siri:StopPointRef>
                                                </PlaceRef>
                                            </Destination>
                                            <Params>
                                                <NumberOfResults>{expected_result_nb}</NumberOfResults>
                                            </Params>
                                        </OJPTripRequest>
                                    </siri:ServiceRequest>
                                </OJPRequest>
                            </OJP>
                            "};
        let token = "eyJvcmciOiI2NDA2NTFhNTIyZmEwNTAwMDEyOWJiZTEiLCJpZCI6IjQ2NWE4N2MwOThkMzRlMzFiN2I5YmRmMDg1MGFjZWQxIiwiaCI6Im11cm11cjEyOCJ9";
        let url = "https://api.opentransportdata.swiss/ojp20";
        let res = client.post(url)
            .header("Content-Type", "application/xml")
            .header("accept", "*/*")
            .bearer_auth(token)
            .body(content)
            .send()
            .await;
        
        let response = match res{
            Ok(res) => {res.text().await},
            Err(e) => {Err(e)}
        }.unwrap();
        println!("{:#?}", response);
        let config = ParserConfig::new()
            .trim_whitespace(false)
            .whitespace_to_characters(true);
        let event_reader = EventReader::new_with_config(response.as_bytes(), config);
        let item = OJPResponse::deserialize(&mut Deserializer::new(event_reader)).unwrap();

        println!("{:#?}", item.ojp_response.siri_service_delivery.producer_ref);
        let ref_trip = &item.ojp_response.siri_service_delivery.o_j_p_trip_delivery.trip_result.
            iter().min_by(|i: &&TripResult, j: &&TripResult| -> Ordering {
            let a = (i.trip.end_time - i.trip.start_time).as_seconds_f64();
            let b = (j.trip.end_time - j.trip.start_time).as_seconds_f64();
            a.partial_cmp(&b).unwrap()
        }).unwrap().trip;
        let min_duration = &ref_trip.duration;
        println!("{:#?}", min_duration);
        // Do the path search
        
        let our_route = plan_journey(
            &hrdf,
            from_point_ref,
            to_point_ref,
            create_date_time(journey_year, journey_month, journey_day, journey_hour, journey_minute),
            true,
        ).unwrap();

        // compare path duration
        println!("{:#?}",our_route.arrival_at());
        println!("{:#?}",ref_trip.end_time);
    }
}
