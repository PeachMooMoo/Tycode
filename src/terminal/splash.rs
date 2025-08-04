use std::time::Instant;

// Tiger ASCII art displayed on application startup
pub const TIGER_ASCII: &str = r#"
⠀⠀⠀⣰⣟⠲⠤⣤⣤⣤⠶⢖⣲⣶⡶⢶⣶⣖⡲⠶⣤⣤⣤⡤⠖⡛⣆⠀⠀⠀
⠀⠀⠀⡏⣿⣷⣄⠀⡟⢡⡶⠛⠉⠁⠀⠀⠈⠉⠛⢶⡌⠻⠀⣠⣾⣿⢹⠀⠀⠀
⠀⠀⠀⡇⢹⣿⣿⠆⣠⠞⢁⣀⣠⣤⡴⢦⣤⣄⣀⡈⠳⣄⢰⣿⣿⣟⢸⡄⠀⠀
⠀⠀⠀⢻⣤⡻⠁⡸⢃⠜⠋⠉⠉⣠⠀⠐⣄⠉⠉⠙⠢⡘⢧⡙⣿⣣⡿⠀⠀⠀
⠀⠀⢀⣾⡷⠁⠊⠀⠀⠤⠖⠋⠉⠑⡀⢀⠊⠉⠙⠲⠤⠀⠀⠑⠀⢾⣷⡄⠀⠀
⠀⠀⣴⡿⠃⠀⡀⣀⡴⠁⣤⠶⠚⠋⠀⠀⠙⠓⠶⣤⠈⢦⣀⢀⠀⠘⢿⣦⠀⠀
⢀⣾⠏⠀⣰⡟⢰⢏⣀⡐⠁⠀⠀⠀⠀⠀⠀⠀⡀⠈⢂⣀⡙⡆⢻⣆⠀⠹⣷⡀
⣼⡏⠀⠀⣿⣧⠸⠀⠻⣏⠟⣾⣄⠀⠀⠀⠀⣠⣷⠻⣹⠟⠀⠇⣼⣷⡀⠀⢹⣷
⣿⣰⠀⠀⣿⣿⡇⠀⠀⠉⠉⢹⣿⠀⠀⠀⠀⣿⡏⠉⠉⠀⠀⢸⣿⣿⠁⠀⣆⣿
⢻⢿⣠⠀⠀⣿⣯⠁⠀⠀⢀⡞⠀⠀⠀⠀⠀⠈⢷⡀⠀⠀⠊⣽⣿⠁⠀⡀⡿⡟
⠈⢸⣿⡆⡀⠈⢿⣇⡀⠀⡼⢰⠀⠀⠀⠀⠀⠀⡏⢧⠀⢀⣸⡿⠃⢀⢰⣿⡗⠀
⠀⠈⢿⢿⣿⣦⡈⠻⢿⣄⡁⡾⠀⠀⠀⠀⠀⠀⢷⢈⣠⡿⠟⢁⣴⣿⡿⡻⠁⠀
⠀⠀⠀⠈⠻⠟⢿⣶⣤⣿⢇⢳⡀⠀⠀⠀⠀⢀⡞⡸⣿⣤⣶⡿⠻⠟⠁⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⣘⣿⣒⣂⠙⠛⢷⡾⠛⠋⢐⣒⣿⣓⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠚⣧⣖⣀⣀⣬⣧⣀⣀⣲⣽⠃⠒⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠉⠛⠳⢤⣄⣠⡤⠾⠛⠉⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"#;

// Tycode ASCII art for welcome message
pub const TYCODE_ASCII: &str = r#"
 ████████ ██    ██  ██████  ██████  ██████  ███████ 
    ██     ██  ██  ██      ██    ██ ██   ██ ██      
    ██      ████   ██      ██    ██ ██   ██ █████   
    ██       ██    ██      ██    ██ ██   ██ ██      
    ██       ██     ██████  ██████  ██████  ███████ "#;

pub struct SplashScreen {
    pub active: bool,
    pub start_time: Instant,
}

impl SplashScreen {
    pub fn new() -> Self {
        Self {
            active: true, // Enable splash screen to show tiger ASCII art
            start_time: Instant::now(),
        }
    }

    // Auto-dismiss after 5 seconds
    pub fn should_auto_dismiss(&self) -> bool {
        self.active && self.start_time.elapsed().as_secs() >= 5
    }

    pub fn dismiss(&mut self) {
        self.active = false;
    }
}
