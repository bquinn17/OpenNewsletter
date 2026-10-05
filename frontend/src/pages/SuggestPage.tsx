import { useNavigate, useParams } from "react-router-dom";
import { PageHeader } from "../components/layout/PageHeader";
import { SuggestQuestionForm } from "../components/candidates/SuggestQuestionForm";
import { useToasts } from "../state/toast";

export function SuggestPage() {
  const { groupId = "" } = useParams();
  const navigate = useNavigate();
  const pushToast = useToasts((s) => s.push);

  return (
    <div className="bg-cream pb-12">
      <PageHeader
        eyebrow="For next month"
        title="Suggest a question"
        back={`/g/${groupId}/upcoming`}
      />

      <SuggestQuestionForm
        groupId={groupId}
        onSuccess={(isAnonymous) => {
          pushToast(
            isAnonymous ? "Question submitted anonymously." : "Question submitted.",
            "success",
          );
          navigate(`/g/${groupId}/upcoming`);
        }}
      />
    </div>
  );
}
