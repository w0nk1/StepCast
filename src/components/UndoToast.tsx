type UndoToastProps = {
  message: string;
  onUndo: () => void;
  onDismiss: () => void;
};

export default function UndoToast({ message, onUndo, onDismiss }: UndoToastProps) {
  return (
    <div className="editor-undo-toast">
      <span className="editor-undo-message">{message}</span>
      <button className="editor-undo-btn" onClick={onUndo}>
        Undo
      </button>
      <button className="editor-undo-dismiss" onClick={onDismiss} title="Dismiss">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
          <path d="M18 6L6 18M6 6l12 12" />
        </svg>
      </button>
    </div>
  );
}
