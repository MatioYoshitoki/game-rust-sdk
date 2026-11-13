//! 训练模式房主有限管理工具（SDK-5-98，机制 106）：协调器层。
//!
//! 受信宿主 RPC（`set_skill_loadout` 同款管道）。开关：`members.training_tools.enabled`
//! 且训练配置（`training_targets`）存在；否则 `Unsupported`。`request_id` 幂等。
use super::*;
use crate::protocol::{
    MatchKey, ResetTrainingCooldowns, RestoreTrainingMember, TrainingToolTargets,
};

const MAX_TRACKED_REQUESTS: usize = 256;

#[derive(Clone, Copy)]
enum ToolKind {
    ResetCooldowns,
    Restore,
}

impl TwoMembers {
    pub(crate) fn reset_training_cooldowns(
        &mut self,
        request: ResetTrainingCooldowns,
    ) -> Result<Status> {
        self.training_tool(
            ToolKind::ResetCooldowns,
            request.key,
            request.request_id,
            request.targets,
        )
    }

    pub(crate) fn restore_training_member(
        &mut self,
        request: RestoreTrainingMember,
    ) -> Result<Status> {
        self.training_tool(
            ToolKind::Restore,
            request.key,
            request.request_id,
            request.targets,
        )
    }

    fn training_tool(
        &mut self,
        kind: ToolKind,
        key: MatchKey,
        request_id: String,
        targets: TrainingToolTargets,
    ) -> Result<Status> {
        if key != self.create.key {
            return Err(Error::StaleMatch);
        }
        if matches!(self.phase, Phase::Closed | Phase::Faulted) {
            return Err(Error::Closed);
        }
        let tools = self.config.training_tools().filter(|tools| tools.enabled);
        let clears = tools
            .map(|tools| tools.restore_clears.clone())
            .unwrap_or_default();
        if self.config.training_targets().is_none()
            || self.config.mechanism_version != 106
            || tools.is_none()
        {
            return Err(Error::Unsupported);
        }
        if self
            .training_tool_requests
            .iter()
            .any(|seen| seen == &request_id)
        {
            return Ok(self.status());
        }
        if self.training_tool_requests.len() >= MAX_TRACKED_REQUESTS {
            self.training_tool_requests.remove(0);
        }
        self.training_tool_requests.push(request_id);
        let seats: Vec<usize> = match &targets {
            TrainingToolTargets::All => (0..self.seats.len()).collect(),
            TrainingToolTargets::Participant { participant_id } => vec![
                self.seats
                    .iter()
                    .position(|s| s.original.participant_id == *participant_id)
                    .ok_or(Error::Unauthorized)?,
            ],
        };
        self.reserve()?;
        let mut events = vec![];
        let world = self.world.as_mut().ok_or(Error::NotReady)?;
        match kind {
            ToolKind::ResetCooldowns => {
                world.reset_training_cooldowns(&seats, &self.config, &mut events)?;
            }
            ToolKind::Restore => {
                world.restore_training_member(
                    &seats,
                    &clears,
                    &self.config,
                    self.now,
                    &mut events,
                )?;
            }
        }
        self.events(events);
        self.dirty = true;
        Ok(self.status())
    }
}
