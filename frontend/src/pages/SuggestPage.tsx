import { useNavigate, useParams } from "react-router-dom";
import { useCycleCadence } from "../api/queries";
import { PageHeader } from "../components/layout/PageHeader";
import { SuggestQuestionForm } from "../components/candidates/SuggestQuestionForm";
import { useToasts } from "../state/toast";
import { cadenceNoun } from "../utils/dates";

export function SuggestPage() {
  const { groupId = "" } = useParams();
  const navigate = useNavigate();
  const pushToast = useToasts((s) => s.push);
  const { frequency } = useCycleCadence();

  return (
    <div className="pb-12">
      <PageHeader
        eyebrow={`For next ${cadenceNoun(frequency)}`}
        title="Suggest a question"
        back={`/g/${groupId}/upcoming`}
        phase="voting"
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
