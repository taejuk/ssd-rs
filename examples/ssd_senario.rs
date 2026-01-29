use ssd_rs::block::*;
use ssd_rs::mapping::*;
use ssd_rs::page::*;
use ssd_rs::ssd_basic::*;
fn main() {
    // 블록 5개, LBA 100개짜리 SSD 생성
    let mut my_ssd = SSD::new(3, 100);

    for i in 0..100 {
        let res = my_ssd.write(0, i);
        //my_ssd.print_blocks();
        match res {
            Ok(_) => {},
            Err(err) => {panic!("error!!!!!");}
        };
    }
    
}