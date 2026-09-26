interface OpenInstanceButtonProps {
  /** 無効化する理由。開ける状態なら null */
  disabledReason: string | null;
  onClick: () => void;
}

/** VRChat でインスタンスを開く button。無効時は hover で理由を表示する */
export function OpenInstanceButton({ disabledReason, onClick }: OpenInstanceButtonProps) {
  const isDisabled = disabledReason !== null;
  return (
    <span className={`open-button-wrapper ${isDisabled ? "is-disabled" : ""}`} title={disabledReason ?? undefined}>
      <button className="btn-accent open-button" onClick={onClick} disabled={isDisabled}>
        Open
      </button>
    </span>
  );
}
