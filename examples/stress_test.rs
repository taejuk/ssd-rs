use ssd_rs::block::*;
use ssd_rs::mapping::*;
use ssd_rs::page::*;
use ssd_rs::ssd_basic::*;

fn main() {
    let mut ssd = SSD::new(5, 100);

    // 2. 혹독한 테스트 (LBA 0~99를 랜덤하게 계속 덮어씀)
    for i in 0..1000 {
        let lba = i % 100; // 0 ~ 99 반복
        let data = i as u32;

        // 에러 나면 즉시 멈춤
        ssd.write(lba, data).expect("SSD Write Failed!");
        println!("{}", i);
        // 100번마다 상태 출력
        if i % 100 == 0 {
            //println!("=== Cycle {} Completed ===", i);
            // 여기에 전체 블록 상태를 요약해서 보여주는 함수가 있으면 좋음
            // ssd.print_status(); 
        }
    }
    println!("waf: {}", ssd.get_waf());
    println!("Test Passed! SSD survived.");
}