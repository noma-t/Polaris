import { MOCK_RECOMMENDED_GROUPS } from "../data/mock";

export function RecommendScreen({ onOpenGroupPage }: { onOpenGroupPage: (groupName: string) => void }) {
  // 未参加のグループを先に表示する
  const groups = [...MOCK_RECOMMENDED_GROUPS].sort((a, b) => Number(a.joined) - Number(b.joined));
  return (
    <div className="recommend-grid">
      {groups.map((group) => (
        <article key={group.name} className="recommend-card">
          <div className="recommend-card-heading">
            <span className="recommend-card-name">{group.name}</span>
            <div className="recommend-card-meta">
              <span>{group.members} members</span>
              <span className={group.joined ? "is-joined" : ""}>{group.joined ? "Joined" : "Not joined"}</span>
            </div>
          </div>
          <div className="recommend-card-description">{group.description}</div>
          <button className="btn-accent-outline recommend-open-button" onClick={() => onOpenGroupPage(group.name)}>
            Open Group Page
          </button>
        </article>
      ))}
    </div>
  );
}
