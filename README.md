# In The Loop

A **local-only desktop application** for macOS that helps you stay "in the loop" on various async work items. Monitor Slack threads, GitHub Actions, Pull Requests, Copilot Agent sessions, and CLI commands - all from your menu bar.

![In The Loop Dashboard](docs/screenshot-placeholder.png)

## Installing and Opening

Download the latest `.dmg` from [Releases](https://github.com/marek-szkudelski/in-the-loop/releases), open it, and drag **InTheLoop** to your Applications folder.

On first launch macOS will block the app because it is not notarized. To open it:

1. Find **InTheLoop** in Finder (e.g. in `/Applications`)
2. Hold **Ctrl** and **left-click** the app icon
3. Select **Open** from the context menu
4. In the dialog that appears, click **Open**

macOS remembers this choice — subsequent launches work normally.

## Features

- 🔔 **System Tray Integration** - Lives in your macOS menu bar
- 📊 **Unified Dashboard** - Track all your async work items in one place
- 🔄 **Auto-Polling** - Configurable background checks for updates
- 💬 **Slack Threads** - Monitor thread replies
- ⚙️ **GitHub Actions** - Track workflow run status
- 🔀 **Pull Requests** - Watch for reviews and status changes
- 💻 **CLI Session Tracking** - Wrap commands to track completion
- 🔒 **Local Storage** - All data stored locally in SQLite, no cloud sync
- 🎨 **Native UI** - Built with Tauri for native performance

## Prerequisites

Before installing, make sure you have:

- **macOS** 10.15 or later
- **Node.js** 18+ and npm
- **Rust** 1.70+ (with cargo)
- **GitHub CLI** (optional, for Copilot Agent tracking)

## Installation

### 1. Clone and Build

```bash
git clone https://github.com/yourusername/in-the-loop.git
cd in-the-loop

# Install frontend dependencies
npm install

# Build and run the app
npm run tauri dev

# Or build for production
npm run tauri build
```

### 2. Configure API Tokens

Once the app is running:

1. Click the app icon in your menu bar
2. Click "Show Settings"
3. Enter your tokens:
   - **Slack User Token** - Get from https://api.slack.com/authentication/token-types#user
   - **GitHub Personal Access Token** - Generate at https://github.com/settings/tokens with `repo` and `workflow` scopes

### 3. Install CLI Wrapper (Optional)

To track CLI commands:

```bash
# Create a bin directory in your home folder
mkdir -p ~/bin

# Copy the wrapper script
cp scripts/loop-track.sh ~/bin/loop-track
chmod +x ~/bin/loop-track

# Add to your PATH (add this to ~/.zshrc or ~/.bashrc)
export PATH="$HOME/bin:$PATH"
```

## Usage

### Adding Items to Track

1. Open the app dashboard
2. Paste a URL in the input field:
   - Slack thread: `https://workspace.slack.com/archives/C12345/p1234567890`
   - GitHub Action: `https://github.com/owner/repo/actions/runs/12345`
   - Pull Request: `https://github.com/owner/repo/pull/42`
3. Optionally add a custom title
4. Click "Add Item"

The app will automatically:
- Detect the item type
- Start polling for updates
- Show notifications when status changes

### Using the CLI Wrapper

Wrap any command to track its execution:

```bash
# Track a long-running command
loop-track npm run build

# Track a CLI tool
loop-track gh copilot suggest "how to use rust async"

# Track tests
loop-track npm test
```

The command will appear in your dashboard as "CLI Session" and update to "completed" or "failed" when done.

### Configuring Polling

1. Open Settings
2. Adjust the "Polling Interval" slider (15s - 5min)
3. Click "Save Settings"

The app will use this interval to check all tracked items.

## Architecture

```
in-the-loop/
├── src/                    # React frontend
│   ├── components/         # UI components
│   ├── services/           # Frontend API wrappers
│   ├── types.ts            # TypeScript interfaces
│   └── main.tsx            # Entry point
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── main.rs         # Tauri app setup & tray
│   │   ├── lib.rs          # Module exports
│   │   ├── db.rs           # SQLite operations
│   │   ├── commands.rs     # Tauri IPC handlers
│   │   ├── polling.rs      # Background polling
│   │   ├── local_server.rs # HTTP server for CLI wrapper
│   │   └── services/       # API clients
│   └── tauri.conf.json     # Tauri configuration
└── scripts/
    └── loop-track.sh       # CLI wrapper script
```

### Data Flow

1. **User adds URL** → Frontend validates → Backend parses URL and stores in SQLite
2. **Polling loop** → Backend checks items every N seconds → Updates database on changes
3. **Status changes** → Backend emits event → Frontend updates UI → System notification
4. **CLI wrapper** → Posts to local HTTP server → Creates item → Updates on completion

### Database Schema

Items are stored in SQLite with:
- `id` - Unique identifier
- `type` - Item type (slack_thread, github_action, github_pr, etc.)
- `status` - Current status (waiting, in_progress, updated, completed, failed)
- `metadata` - JSON blob with type-specific data
- `last_checked_at` - Last poll timestamp
- `created_at` - When item was added

## Development

### Running in Dev Mode

```bash
npm run dev
```

This starts:
- Vite dev server on port 1420
- Tauri app in debug mode
- Hot reload for frontend changes
- Rust recompilation on backend changes

### Building for Production

```bash
npm run tauri build
```

Creates a `.dmg` installer in `src-tauri/target/release/bundle/`

### Running Tests

```bash
# Rust tests
cd src-tauri
cargo test

# Frontend tests (if added)
npm test
```

## Troubleshooting

### App doesn't start
- Check that ports 1420 and 19532 are available
- Verify Rust and Node.js are properly installed
- Check logs in console

### No updates showing
- Verify API tokens are correctly configured
- Check token permissions (Slack: user token, GitHub: repo + workflow scopes)
- Increase polling interval if hitting rate limits
- Check item metadata is correctly parsed

### CLI wrapper not working
- Ensure the app is running (local server must be active)
- Verify `loop-track` is in your PATH
- Check port 19532 is accessible

## Contributing

Contributions welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

MIT License - see LICENSE file for details

## Acknowledgments

- Built with [Tauri](https://tauri.app/)
- Uses [React](https://react.dev/) for the frontend
- Database powered by [SQLite](https://www.sqlite.org/)
