export function AboutScreen({ version }: { version: string }) {
  const rows = [
    { title: "このアプリについて", body: "V睡インスタンスを探すための非公式の個人制作ツールです。VRChat Inc.とは提携していません。" },
    { title: "ネットワーク", body: "VRChatのサーバーにのみ接続します。ユーザーデータの収集や外部送信は行いません。" },
    { title: "このデバイスに保存されるデータ", body: "認証情報・フレンドの添い寝フラグ情報" },
    { title: "絶対に行わないこと", body: "インスタンスへの自動参加や招待の自動送信" },
    {
      title: "免責事項",
      body: "ご利用は自己責任でお願いします。このツールの使用により生じたいかなる問題についても、開発者は責任を負いません。",
    },
    {
      title: "フォント",
      body: "Baloo 2 (Copyright 2019 The Baloo 2 Project Authors), IBM Plex Sans JP および IBM Plex Mono (Copyright © 2017 IBM Corp.)。SIL Open Font License, Version 1.1 のもとで提供されています — https://openfontlicense.org",
    },
    { title: "バージョン", body: version },
  ];
  return (
    <div className="about-list">
      {rows.map((row) => (
        <div key={row.title} className="about-row">
          <span className="about-row-title">{row.title}</span>
          <span className="about-row-body">{row.body}</span>
        </div>
      ))}
    </div>
  );
}
