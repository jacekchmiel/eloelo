import CampaignIcon from "@mui/icons-material/Campaign";
import { IconButton, type IconButtonProps } from "@mui/material";
import { invoke } from "../Api";
import type { Side } from "../model";
import { DefaultTooltip } from "./DefaultTooltip";

export function CallPlayerButton({
  playerKey,
  side,
  ...props
}: { side: Side; playerKey: string } & IconButtonProps) {
  return (
    <DefaultTooltip title="Call to lobby">
      <IconButton
        {...props}
        edge={side === "left" ? "start" : "end"}
        onClick={async () => {
          await invoke("call_player", {
            id: playerKey,
          });
        }}
      >
        <CampaignIcon />
      </IconButton>
    </DefaultTooltip>
  );
}
