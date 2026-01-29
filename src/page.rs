use std::fmt;

// 1. PageState에도 Debug가 있어야 출력 가능하므로 추가합니다.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum PageState {
    Free,
    Valid,
    Invalid,
}

#[derive(Clone)] // 2. 여기서 Debug를 제거하고 직접 구현(impl)합니다.
pub struct Page {
    pub content: u32,
    pub state: PageState,
}

// 3. Debug 트레이트 수동 구현
impl fmt::Debug for Page {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.state {
            // Free 상태일 때는 데이터가 의미 없으므로 상태만 깔끔하게 출력
            PageState::Free => write!(f, "[  FREE   ]"),
            
            // Invalid 상태일 때는 (구버전 데이터)임을 표시
            PageState::Invalid => write!(f, "[ INVALID ] (trash: {:#010X})", self.content),
            
            // Valid 상태일 때는 데이터를 16진수로 예쁘게 출력
            PageState::Valid => write!(f, "[  VALID  ] Data: {:#010X}", self.content),
        }
    }
}
