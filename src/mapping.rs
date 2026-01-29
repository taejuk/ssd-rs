use std::fmt;

// 물리 주소를 표현하는 구조체 (어느 블록, 어느 페이지인지)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicalAddress {
    pub block_id: u32,
    pub page_offset: usize,
}

pub struct MappingTable {
    // 인덱스(Index)가 곧 LBA(Logical Block Address)입니다.
    // 값(Value)은 해당 LBA가 저장된 물리 주소(PBA)입니다.
    // Option::None이면 "아직 데이터가 안 쓰인 주소(Unmapped)"라는 뜻입니다.
    entries: Vec<Option<PhysicalAddress>>,
}

impl MappingTable {
    // 1. 테이블 생성: SSD 용량(총 LBA 개수)만큼 빈 테이블을 만듭니다.
    pub fn new(total_lbas: usize) -> Self {
        MappingTable {
            entries: vec![None; total_lbas],
        }
    }

    // 2. 조회 (Read): LBA를 주면 PBA를 반환합니다.
    pub fn get(&self, lba: usize) -> Option<PhysicalAddress> {
        if lba >= self.entries.len() {
            panic!("LBA {} is out of range!", lba);
        }
        self.entries[lba]
    }

    // 3. 업데이트 (Write): LBA의 위치를 새로운 PBA로 바꿉니다.
    // [중요] 리턴값: 만약 이 LBA에 예전 데이터가 있었다면, 그 구버전 PBA를 리턴해줍니다.
    // 왜? -> 구버전 PBA 위치에 가서 "너 이제 쓰레기(Invalid)야"라고 마킹해야 하니까요!
    pub fn update(&mut self, lba: usize, new_pba: PhysicalAddress) -> Option<PhysicalAddress> {
        if lba >= self.entries.len() {
            panic!("LBA {} is out of range!", lba);
        }

        let old_pba = self.entries[lba]; // 기존에 가리키던 주소 (없으면 None)
        self.entries[lba] = Some(new_pba); // 새 주소로 갱신
        
        old_pba // 옛날 주소 반환 (GC 처리를 위해 필수)
    }
    
    // 4. 매핑 해제 (Trim/Unmap): 파일을 지웠을 때 사용
    pub fn unmap(&mut self, lba: usize) -> Option<PhysicalAddress> {
         if lba >= self.entries.len() {
            panic!("LBA {} is out of range!", lba);
        }
        
        let old_pba = self.entries[lba];
        self.entries[lba] = None;
        old_pba
    }

    pub fn entries(&self) -> &Vec<Option<PhysicalAddress>> {
        &self.entries
    }
}

// PhysicalAddress는 간단하므로 기존 derive를 유지해도 되지만, 
// MappingTable 내부에서 예쁘게 찍기 위해 Display를 추가하면 더 좋습니다.
// (선택 사항: 없어도 아래 코드에서 직접 포맷팅하면 됩니다)

impl fmt::Debug for MappingTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 1. 전체 매핑 개수 계산
        let total_mapped = self.entries.iter().filter(|e| e.is_some()).count();
        let total_lbas = self.entries.len();

        writeln!(f, "=== Mapping Table Summary ===")?;
        writeln!(f, "  Usage: {} / {} LBAs mapped", total_mapped, total_lbas)?;
        writeln!(f, "  ---------------------------")?;

        // 2. 매핑된 항목만 골라서 출력 (Sparse View)
        // 만약 매핑된 게 너무 많으면 앞부분 50개만 보여주는 식으로 제한을 둘 수도 있습니다.
        let mut count = 0;
        for (lba, entry) in self.entries.iter().enumerate() {
            if let Some(pba) = entry {
                // 보기 좋게 정렬: LBA는 5자리 확보, 화살표, PBA 정보
                writeln!(
                    f, 
                    "  LBA [{:<5}] -> Block {:<4} | Page {:<3}", 
                    lba, pba.block_id, pba.page_offset
                )?;
                
                count += 1;
                // (선택) 로그가 너무 길어지는 것을 방지하려면 주석 해제
                // if count >= 20 {
                //     writeln!(f, "  ... (remaining entries hidden) ...")?;
                //     break;
                // }
            }
        }

        if total_mapped == 0 {
            writeln!(f, "  (Table is Empty)")?;
        }

        writeln!(f, "=============================")
    }
}