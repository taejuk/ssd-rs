use std::fs::File;
use std::io::BufReader;
use serde::Deserialize;
use rand::Rng;

use ssd_rs::ssd_basic::SSD as SSD_BASIC; 
use ssd_rs::ssd_wear_leveling::SSD as SSD_WEAR_LEVELING;
use ssd_rs::block::PAGES_PER_BLOCK;

#[derive(Deserialize, Debug)]
struct TestArg {
    num_blocks: usize,
    num_lbas: usize,
}

fn main() {
    // 1. JSON 파일 열기
    let file = File::open("./test/waf.json").expect("Failed to open waf_test.json");
    let reader = BufReader::new(file);

    // 2. JSON 파싱 (Vec<TestArg>로 변환)
    let args: Vec<TestArg> = serde_json::from_reader(reader).expect("Failed to parse JSON");

    println!("=== Wear leveling Test Explosion Experiment Start ===\n");

    // 3. 각 테스트 케이스 실행
    for (i, arg) in args.iter().enumerate() {
        println!(">>> Running Test Case #{}", i + 1);
        
        let mut ssd = SSD_BASIC::new(arg.num_blocks, arg.num_lbas);
        let mut ssd_wear_leveling = SSD_WEAR_LEVELING::new(arg.num_blocks, arg.num_lbas);
        // (2) 환경 정보 계산 및 출력
        let total_physical_pages = arg.num_blocks * PAGES_PER_BLOCK;
        let op_ratio = (total_physical_pages as f64 - arg.num_lbas as f64) / arg.num_lbas as f64 * 100.0;
        
        println!("    Config: Blocks = {}, LBAs = {}", arg.num_blocks, arg.num_lbas);
        println!("    Physical Pages: {}, Logical Pages: {}", total_physical_pages, arg.num_lbas);
        println!("    Over-Provisioning (OP): {:.2}%", op_ratio);

        
        let iterations = arg.num_lbas * 100; 
        let mut rng = rand::thread_rng();

        // 진행바 느낌을 위해...
        //print!("    Writing data... ");
        
        for _ in 0..iterations {
            // 랜덤한 LBA 선택 (Hot/Cold 구분 없이 완전 랜덤)
            let target_lba = rng.gen_range(0..arg.num_lbas);
            let dummy_data = 0xDEADBEEF;

            // 쓰기 수행 (에러나면 실험 중단)
            if let Err(e) = ssd.write(target_lba, dummy_data) {
                println!("\n    [Error] Write failed: {}", e);
                break;
            }

            if let Err(e) = ssd_wear_leveling.write(target_lba, dummy_data) {
                println!("\n    [Error] Write failed: {}", e);
                break;
            }

        }
        
        // (4) 결과 출력 (WAF 확인)
        let waf = ssd.get_waf();
        println!("    [Result] BASIC");
        println!("    WAF: {:.4}", waf);
        println!("    Wear Leveling: {:?}", ssd.compute_wear_metrics());
        
        let waf_wearleveling = ssd_wear_leveling.get_waf();
        println!("    [Result] BASIC");
        println!("    WAF: {:.4}", waf_wearleveling);
        println!("    Wear Leveling: {:?}", ssd_wear_leveling.compute_wear_metrics());
        
        println!("----------------------------------------\n");

    }
}