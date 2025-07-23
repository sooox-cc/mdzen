# mdzen

> *A minimalist, zen-like markdown reader built with Rust and egui*

[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-dea584.svg?logo=rust)](https://www.rust-lang.org/)

mdzen provides a clean, distraction-free environment for reading markdown files. Inspired by zen philosophy, it focuses on simplicity, elegance, and the pure joy of reading.

## ✨ Features

- **🎨 Beautiful Dark Theme** - Carefully crafted colors optimized for extended reading
- **⚡ Lightning Fast** - Built with Rust and egui for instant responsiveness  
- **🔍 Smart Search** - Find text with highlighting and easy navigation
- **📑 Table of Contents** - Quick navigation through document structure
- **🎯 Syntax Highlighting** - Code blocks rendered with beautiful syntax colors
- **🖼️ Image Support** - Display local and web images inline
- **📱 Drag & Drop** - Simply drop markdown files to open them
- **🔧 Flexible Viewing** - Switch between normal and wide reading modes

## 🚀 Quick Start

### Installation

1. **Clone and build:**
   ```bash
   git clone https://github.com/sooox/mdzen
   cd mdzen
   ./install.sh
   ```

2. **Or build manually:**
   ```bash
   cargo build --release
   # Binary will be in target/release/mdzen
   ```

### Usage

```bash
# Open a specific file
mdzen README.md

# Launch and choose file via GUI
mdzen
```

## 🏗️ Built With

- **[Rust](https://www.rust-lang.org/)** - Systems programming language focused on safety and performance
- **[egui](https://github.com/emilk/egui)** - Immediate mode GUI framework  
- **[pulldown-cmark](https://github.com/raphlinus/pulldown-cmark)** - CommonMark markdown parser
- **[syntect](https://github.com/trishume/syntect)** - Syntax highlighting engine

## 🎯 Design Philosophy

mdzen embraces minimalism:

- **No clutter** - Clean interface that gets out of your way
- **No distractions** - Focus purely on your content
- **No complexity** - Simple, intuitive interaction patterns
- **No compromises** - Fast performance without sacrificing features

## 🔧 Configuration

mdzen works beautifully out of the box, but you can customize:

- **Font size** - Adjust via View menu or `+`/`-` buttons
- **Viewing mode** - Toggle between normal (centered) and wide modes
- **File associations** - Set mdzen as your default markdown viewer

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

1. Fork the project
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request
