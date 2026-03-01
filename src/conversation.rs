use crate::client::glm::Message;

/// Conversation history for the AI chat
pub struct Conversation {
    messages: Vec<Message>,
}

impl Conversation {
    pub fn new(system_content: String) -> Self {
        Self {
            messages: vec![Message {
                role: "system".to_string(),
                content: Some(system_content),
                tool_calls: None,
                tool_call_id: None,
            }],
        }
    }

    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: "user".to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    pub fn add_assistant_message(&mut self, msg: Message) {
        self.messages.push(msg);
    }

    pub fn add_tool_result(&mut self, tool_call_id: &str, result: &str) {
        self.messages.push(Message {
            role: "tool".to_string(),
            content: Some(result.to_string()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.to_string()),
        });
    }

    pub fn get_messages(&self) -> Vec<Message> {
        self.messages.clone()
    }

    pub fn clear_keeping_system(&mut self) {
        if let Some(system) = self.messages.first().cloned() {
            self.messages = vec![system];
        }
    }
}
