import Box from "@mui/material/Box";
import Modal from "@mui/material/Modal";
import type { PropsWithChildren } from "react";

export function DefaultModal({
  show,
  setShow,
  children,
}: PropsWithChildren<{ show: boolean; setShow: (show: boolean) => void }>) {
  return (
    <Modal open={show} onClose={() => setShow(false)}>
      <Box
        sx={{
          position: "absolute" as const,
          top: "5%",
          left: "50%",
          transform: "translate(-50%, 0%)",
          bgcolor: "background.paper",
          boxShadow: 24,
          p: 4,
          overflowY: "auto",
          maxHeight: "90%",
        }}
      >
        {children}
      </Box>
    </Modal>
  );
}
