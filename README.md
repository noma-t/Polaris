<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="128" alt="Polaris" />
</p>

<h1 align="center">Polaris</h1>

<p align="center">
  V睡するインスタンスを探すための Windows 向けデスクトップアプリ
</p>

---

Polaris は、VRChat のフレンドや V睡グループがいまどのインスタンスにいるかを一覧で確認し、そのまま VRChat 内で開けるようにする非公式ツールです。

> [!NOTE]
> Polaris は個人制作の非公式ツールであり、VRChat Inc. とは一切提携していません。

## 主な機能

- **フレンドの居場所を確認** — ピン留めしたフレンドの現在のワールドとステータスをリアルタイムで表示します。
- **グループインスタンスの一覧** — 所属グループで開かれているインスタンスを一覧表示します。表示するグループは自由に選べます。
- **ワンクリックで VRChat 内で開く** — フレンドやグループのインスタンスを、アプリから直接 VRChat で開けます。
- **V睡グループの紹介** — 代表的な V睡グループを一覧で紹介し、VRChat のグループページをブラウザで開けます。
- **VRChat 起動状態の表示** — 画面下部のステータスバーで VRChat が起動中かどうかを確認できます。
- **自動アップデート** — 設定画面から最新版の確認・アップデートができます。

## インストール

1. [Releases](https://github.com/noma-t/Polaris/releases/latest) から最新版のインストーラー（`Polaris_x.x.x_x64-setup.exe`）をダウンロードします。
2. ダウンロードしたインストーラーを実行します。

以降のアップデートは、アプリの **Settings → Updates** から行えます。

## 使い方

1. VRChat アカウントでログインします（2 段階認証にも対応しています）。
2. **Instances** 画面で、表示したいフレンド・グループを各パネル上部の表示切り替えボタンから選びます。
3. 目的のインスタンスの **Open** ボタンを押すと、VRChat でそのインスタンスが開きます。

> [!TIP]
> VRChat を Steam の既定の場所以外にインストールしている場合は、**Settings → VRChat launcher** で `launch.exe` のパスを指定してください。

## プライバシーと安全性

- 通信先は VRChat のサーバーのみです。ユーザーデータの収集や外部への送信は行いません。
- ログイン情報（セッション）は OS の資格情報ストア（Windows 資格情報マネージャー）に保存されます。
- インスタンスへの自動参加や、招待の自動送信は**絶対に行いません**。

## 免責事項

ご利用は自己責任でお願いします。本ツールの使用により生じたいかなる問題についても、開発者は責任を負いません。

---

## 開発者向け情報

### 技術スタック

| 領域 | 使用技術 |
| --- | --- |
| デスクトップフレームワーク | [Tauri 2](https://tauri.app/) |
| フロントエンド | React 19 + TypeScript + Vite |
| バックエンド | Rust（`reqwest` / `tokio-tungstenite` / `keyring` など） |

### 必要な環境

- [Node.js](https://nodejs.org/)（CI では v22 を使用）
- [Rust](https://www.rust-lang.org/tools/install)（stable）
- Tauri の [前提条件](https://tauri.app/start/prerequisites/)（Windows では Microsoft C++ Build Tools と WebView2）

### 開発サーバーの起動

```sh
npm install
npm run tauri dev
```

### ビルド

```sh
npm run tauri build
```

成果物（インストーラー）は `src-tauri/target/release/bundle/` 以下に出力されます。

> [!NOTE]
> `createUpdaterArtifacts` が有効なため、ローカルでのビルドには updater 署名用の秘密鍵（環境変数 `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`）が必要です。

### ディレクトリ構成

```
.
├── src/                 # フロントエンド（React）
│   ├── screens/         # 各画面
│   ├── components/      # UI コンポーネント
│   ├── hooks/           # カスタムフック
│   ├── lib/             # Tauri コマンド呼び出しなどのロジック
│   └── data/            # V睡グループなどの静的データ
├── src-tauri/           # バックエンド（Rust / Tauri）
│   └── src/             # VRChat API クライアント、認証、WebSocket pipeline など
├── scripts/             # 開発・リリース用スクリプト
└── .github/workflows/   # GitHub Actions
```

### リリース手順

1. `src-tauri/tauri.conf.json` の `version` を更新します。
2. `main` ブランチに push すると、GitHub Actions（`.github/workflows/release.yml`）が Windows 向けにビルドし、Draft Release を作成します。
   - リリースノートはコミットメッセージから `scripts/generate-release-notes.mjs` により自動生成されます。
3. Draft Release の内容を確認して公開します。

コミットメッセージは [Conventional Commits](https://www.conventionalcommits.org/) 形式で、リリースノートに載る前提で日本語で書きます（`feat` → 新機能、`fix` → 不具合修正、`perf` / `refactor` → 改善）。

### 推奨 IDE

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

### ライセンス表記

同梱フォント: Baloo 2 (Copyright 2019 The Baloo 2 Project Authors)、IBM Plex Sans JP / IBM Plex Mono (Copyright © 2017 IBM Corp.)。いずれも [SIL Open Font License 1.1](https://openfontlicense.org) のもとで提供されています。
