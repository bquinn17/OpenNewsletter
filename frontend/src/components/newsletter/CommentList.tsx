import { useEffect, useRef, useState } from "react";
import { Avatar } from "../ui/Avatar";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import {
  useAddComment,
  useConfig,
  useDeleteComment,
  useEditComment,
  useGroup,
} from "../../api/queries";
import { formatRelative } from "../../utils/dates";
import type { Comment } from "../../api/types";

interface Props {
  comments: Comment[];
  totalCount: number;
  onShowAll: () => void;
  groupId: string;
  cycleId: string;
  questionId: string;
  responseId: string;
}

export function CommentList({
  comments,
  totalCount,
  onShowAll,
  groupId,
  cycleId,
  questionId,
  responseId,
}: Props) {
  const { data: config } = useConfig();
  const { data: group } = useGroup(groupId);
  const isAdmin = !!group?.members.find(
    (m) => m.userId === config?.user.userId && m.role === "admin",
  );
  const [draft, setDraft] = useState("");
  const addComment = useAddComment(groupId, cycleId);
  const editComment = useEditComment(groupId, cycleId);
  const deleteComment = useDeleteComment(groupId, cycleId);
  const hidden = totalCount - comments.length;

  const submit = () => {
    const body = draft.trim();
    if (!body) return;
    addComment.mutate({ questionId, responseId, body });
    setDraft("");
  };

  return (
    <div className="mt-4 pt-4 border-t border-line space-y-3 text-sm">
      {comments.map((c) => (
        <CommentRow
          key={c.commentId}
          comment={c}
          isMine={config?.user.userId === c.authorUserId}
          canDelete={config?.user.userId === c.authorUserId || isAdmin}
          onSaveEdit={(body) =>
            editComment.mutateAsync({ questionId, responseId, commentId: c.commentId, body })
          }
          onDelete={() =>
            deleteComment.mutateAsync({ questionId, responseId, commentId: c.commentId })
          }
        />
      ))}
      {hidden > 0 && (
        <button onClick={onShowAll} className="text-grape font-semibold text-sm">
          Show {hidden} more comment{hidden === 1 ? "" : "s"}
        </button>
      )}
      <div className="flex items-center gap-2 mt-2">
        <Avatar
          name={config?.user.displayName ?? "?"}
          color={config?.user.avatarColor}
          url={config?.user.avatarUrl}
          size="xs"
        />
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              submit();
            }
          }}
          className="flex-1 min-w-0 bg-cream border border-line rounded-full px-4 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
          placeholder="Add a comment…"
        />
        <button
          type="button"
          onClick={submit}
          disabled={!draft.trim() || addComment.isPending}
          className="rounded-full bg-grape text-white text-xs font-bold px-4 py-2 disabled:opacity-40 disabled:cursor-not-allowed"
          aria-label="Post comment"
        >
          {addComment.isPending ? "…" : "Post"}
        </button>
      </div>
    </div>
  );
}

function CommentRow({
  comment,
  isMine,
  canDelete,
  onSaveEdit,
  onDelete,
}: {
  comment: Comment;
  isMine: boolean;
  canDelete: boolean;
  onSaveEdit: (body: string) => Promise<unknown>;
  onDelete: () => Promise<unknown>;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(comment.body);
  const [menuOpen, setMenuOpen] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!menuOpen) return;
    function onDoc(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) setMenuOpen(false);
    }
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [menuOpen]);

  const save = async () => {
    const next = draft.trim();
    if (!next || next === comment.body) {
      setEditing(false);
      return;
    }
    setBusy(true);
    try {
      await onSaveEdit(next);
      setEditing(false);
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setBusy(true);
    try {
      await onDelete();
      setConfirmDelete(false);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="group">
      <div className="flex items-baseline gap-2">
        <span className="font-semibold">{comment.authorDisplayName}</span>
        <span className="text-xs text-inkmuted">{formatRelative(comment.createdAt)}</span>
        {comment.editedAt && <span className="text-xs text-inkmuted italic">(edited)</span>}
        {(isMine || canDelete) && (
          <div ref={menuRef} className="ml-auto relative">
            <button
              onClick={() => setMenuOpen((s) => !s)}
              disabled={busy}
              className="text-inkmuted hover:text-ink px-1.5 leading-none disabled:opacity-50"
              aria-label="Comment options"
            >
              ⋯
            </button>
            {menuOpen && (
              <div className="absolute right-0 top-6 z-30 w-36 bg-white rounded-2xl shadow-pop border border-line p-1 animate-pop">
                {isMine && (
                  <button
                    onClick={() => {
                      setDraft(comment.body);
                      setEditing(true);
                      setMenuOpen(false);
                    }}
                    className="w-full text-left px-3 py-2 rounded-xl text-sm font-semibold hover:bg-cream"
                  >
                    Edit
                  </button>
                )}
                {canDelete && (
                  <button
                    onClick={() => {
                      setMenuOpen(false);
                      setConfirmDelete(true);
                    }}
                    className="w-full text-left px-3 py-2 rounded-xl text-sm font-semibold text-coral hover:bg-cream"
                  >
                    Delete
                  </button>
                )}
              </div>
            )}
          </div>
        )}
      </div>
      {editing ? (
        <div className="mt-1">
          <textarea
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            disabled={busy}
            maxLength={2000}
            className="w-full bg-cream border border-line rounded-2xl px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30 min-h-[60px]"
          />
          <div className="flex justify-end gap-2 mt-1.5">
            <button
              type="button"
              onClick={() => setEditing(false)}
              disabled={busy}
              className="text-xs text-inkmuted font-semibold disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={save}
              disabled={busy || !draft.trim()}
              className="text-xs text-grape font-bold disabled:opacity-50"
            >
              {busy ? "Saving…" : "Save"}
            </button>
          </div>
        </div>
      ) : (
        <>
          {comment.body && <p className="text-ink/80">{comment.body}</p>}
          {comment.image?.displayUrl && (
            <img
              src={comment.image.displayUrl}
              alt={comment.image.alt ?? ""}
              className="mt-1.5 rounded-2xl max-h-72 object-contain bg-cream"
            />
          )}
        </>
      )}
      <ConfirmDialog
        open={confirmDelete}
        title="Delete this comment?"
        message="This can't be undone."
        confirmLabel="Delete"
        tone="danger"
        busy={busy}
        onConfirm={() => void remove()}
        onCancel={() => setConfirmDelete(false)}
      />
    </div>
  );
}
