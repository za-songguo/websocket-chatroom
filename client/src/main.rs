use gloo::console;
use web_sys::{
    js_sys::JsString,
    wasm_bindgen::{closure::Closure, JsCast},
    CloseEvent, HtmlInputElement, MessageEvent, WebSocket,
};
use yew::prelude::*;

const WS_URL: &str = "ws://127.0.0.1:8080/";

fn main() {
    yew::Renderer::<App>::new().render();
}

#[function_component]
fn App() -> Html {
    let message_input_ref = use_node_ref();
    let cloned_ws = use_mut_ref(|| None);
    let messages = use_state(Vec::new);

    {
        let cloned_ws = cloned_ws.clone();

        use_effect_with((), move |_| {
            let ws = WebSocket::new(WS_URL).unwrap();

            let onopen = Closure::<dyn Fn()>::new(|| {
                console::log!("连接成功");
            });
            ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
            onopen.forget();

            let onerror = Closure::<dyn Fn(_)>::new(|e: ErrorEvent| {
                console::log!("出现错误", e);
            });
            ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
            onerror.forget();

            let onclose = Closure::<dyn Fn(_)>::new(|e: CloseEvent| {
                console::log!("连接关闭", e);
            });
            ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
            onclose.forget();

            cloned_ws.borrow_mut().replace(ws);
        });
    }

    {
        let cloned_ws = cloned_ws.clone();
        let messages = messages.clone();

        use_effect_with(messages.clone(), move |_| {
            let onmessage = Closure::<dyn Fn(_)>::new(move |e: MessageEvent| {
                if let Ok(new_message) = e.data().dyn_into::<JsString>() {
                    console::log!("收到消息", &new_message);
                    append_message(messages.clone(), new_message.into());
                } else {
                    append_message(messages.clone(), "（非文本消息）".into());
                }
            });
            cloned_ws
                .as_ref()
                .borrow_mut()
                .clone()
                .unwrap()
                .set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
            onmessage.forget();
        })
    }

    let onclick = {
        let message_input_ref = message_input_ref.clone();
        let cloned_ws = cloned_ws.clone();
        let messages = messages.clone();

        Callback::from(move |_| {
            let new_message = message_input_ref
                .cast::<HtmlInputElement>()
                .unwrap()
                .value();

            if cloned_ws.as_ref().borrow().clone().unwrap().ready_state() == 1
                && !new_message.is_empty()
            {
                if let Err(e) = cloned_ws
                    .as_ref()
                    .borrow_mut()
                    .clone()
                    .unwrap()
                    .send_with_str(&new_message)
                {
                    console::log!("消息发送失败", e)
                } else {
                    append_message(messages.clone(), new_message);
                }
            } else {
                console::log!("消息发送失败，连接已关闭");
            }
        })
    };

    html! {
       <div>
            <form>
                <input type="text" ref={message_input_ref} placeholder="输入消息......" style="width: 30%; display: block; margin-top: 1%; margin-left: 1%;"/>
                <input type="button" {onclick} class="button" style="margin-top: 1%; margin-left: 1%;" value="发送"/>
            </form>
            <div>
                {
                    messages.iter().enumerate().map(|(i, m)| {
                        html!{<p key={i} style="margin-left: 1%;">{m}</p>}
                    }).collect::<Html>()
                }
            </div>
       </div>
    }
}

fn append_message(messages: UseStateHandle<Vec<String>>, new_message: String) {
    let mut temp = (*messages).clone();
    temp.push(new_message);
    messages.set(temp);
}
