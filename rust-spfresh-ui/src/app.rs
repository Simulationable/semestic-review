use gloo_net::http::Request;
use leptos::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq)]
enum Tab { Insert, Bulk, Search }

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct ReviewPayload {
    review_title: String,
    review_body: String,
    product_id: String,
    review_rating: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct InsertRequest { review: ReviewPayload }

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct BulkRequest { reviews: Vec<ReviewPayload> }

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct SearchRequest { query: String, top_k: i32 }

#[component]
pub fn App() -> impl IntoView {
    let (tab, set_tab) = create_signal(Tab::Insert);

    // Insert state
    let (title, set_title) = create_signal(String::new());
    let (body, set_body) = create_signal(String::new());
    let (pid, set_pid) = create_signal(String::new());
    let (rating, set_rating) = create_signal(5);
    let (insert_loading, set_insert_loading) = create_signal(false);
    let (insert_resp, set_insert_resp) = create_signal(String::new());
    let (insert_err, set_insert_err) = create_signal(String::new());

    // Bulk state
    let (bulk_items, set_bulk_items) = create_signal::<Vec<ReviewPayload>>(vec![]);
    let (bulk_loading, set_bulk_loading) = create_signal(false);
    let (bulk_resp, set_bulk_resp) = create_signal(String::new());
    let (bulk_err, set_bulk_err) = create_signal(String::new());

    // Search state
    let (query, set_query) = create_signal(String::new());
    let (top_k, set_top_k) = create_signal(3);
    let (search_loading, set_search_loading) = create_signal(false);
    let (search_resp, set_search_resp) = create_signal(String::new());
    let (search_err, set_search_err) = create_signal(String::new());

    // ---- Actions (ผ่าน proxy => /api/... -> localhost:8000) ----
    let do_insert = move |_| {
        let url = "/api/reviews";
        let payload = InsertRequest { review: ReviewPayload {
            review_title: title.get_untracked(),
            review_body: body.get_untracked(),
            product_id: pid.get_untracked(),
            review_rating: rating.get_untracked(),
        }};
        set_insert_loading.set(true);
        set_insert_err.set(String::new());
        set_insert_resp.set(String::new());
        spawn_local(async move {
            let resp = Request::post(url)
                .header("Content-Type", "application/json")
                .json(&payload).unwrap()
                .send().await;
            match resp {
                Ok(r) => {
                    let status = r.status();           // u16
                    let text = r.text().await.unwrap_or_default();
                    if status >= 400 { set_insert_err.set(format!("HTTP {}: {}", status, text)); }
                    else { set_insert_resp.set(text); }
                }
                Err(e) => set_insert_err.set(format!("fetch error: {}", e)),
            }
            set_insert_loading.set(false);
        });
    };

    let add_bulk_row = move |_| set_bulk_items.update(|v| v.push(ReviewPayload::default()));
    let remove_bulk_row = move |idx: usize| set_bulk_items.update(|v| { if idx < v.len() { v.remove(idx); } });

    let do_bulk = move |_| {
        let url = "/api/reviews/bulk";
        let payload = BulkRequest { reviews: bulk_items.get_untracked() };
        set_bulk_loading.set(true);
        set_bulk_err.set(String::new());
        set_bulk_resp.set(String::new());
        spawn_local(async move {
            let resp = Request::post(url)
                .header("Content-Type", "application/json")
                .json(&payload).unwrap()
                .send().await;
            match resp {
                Ok(r) => {
                    let status = r.status();
                    let text = r.text().await.unwrap_or_default();
                    if status >= 400 { set_bulk_err.set(format!("HTTP {}: {}", status, text)); }
                    else { set_bulk_resp.set(text); }
                }
                Err(e) => set_bulk_err.set(format!("fetch error: {}", e)),
            }
            set_bulk_loading.set(false);
        });
    };

    let do_search = move |_| {
        let url = "/api/search";
        let payload = SearchRequest { query: query.get_untracked(), top_k: top_k.get_untracked() };
        set_search_loading.set(true);
        set_search_err.set(String::new());
        set_search_resp.set(String::new());
        spawn_local(async move {
            let resp = Request::post(url)
                .header("Content-Type", "application/json")
                .json(&payload).unwrap()
                .send().await;
            match resp {
                Ok(r) => {
                    let status = r.status();
                    let text = r.text().await.unwrap_or_default();
                    if status >= 400 { set_search_err.set(format!("HTTP {}: {}", status, text)); }
                    else { set_search_resp.set(text); }
                }
                Err(e) => set_search_err.set(format!("fetch error: {}", e)),
            }
            set_search_loading.set(false);
        });
    };

    view! {
        <div class="wrap">
            <header class="row" style="justify-content:space-between;margin-bottom:16px;">
                <div class="h1">"Reviews Admin Console (Leptos)"</div>
            </header>

            <div class="row tabs" style="margin-bottom:16px;">
                <button class=move || if tab.get() == Tab::Insert {"active"} else {""} on:click=move |_| set_tab.set(Tab::Insert)>"Insert Review"</button>
                <button class=move || if tab.get() == Tab::Bulk {"active"} else {""} on:click=move |_| set_tab.set(Tab::Bulk)>"Bulk Insert"</button>
                <button class=move || if tab.get() == Tab::Search {"active"} else {""} on:click=move |_| set_tab.set(Tab::Search)>"Search"</button>
            </div>

            {move || match tab.get() {
                Tab::Insert => view! {
                    <div class="grid cols-2">
                        <div class="card">
                            <div style="font-weight:600;margin-bottom:8px;">"New Review"</div>
                            <label>
                                <span>"Review Title"</span>
                                <input prop:value=move || title.get() on:input=move |ev| set_title.set(event_target_value(&ev)) />
                            </label>
                            <label>
                                <span>"Review Body"</span>
                                <textarea on:input=move |ev| set_body.set(event_target_value(&ev))>{move || body.get()}</textarea>
                            </label>
                            <div class="row">
                                <label style="flex:1">
                                    <span>"Product ID"</span>
                                    <input prop:value=move || pid.get() on:input=move |ev| set_pid.set(event_target_value(&ev)) />
                                </label>
                                <label style="width:140px">
                                    <span>"Rating (1-5)"</span>
                                    <input type="number" prop:value=move || rating.get().to_string() on:input=move |ev| {
                                        if let Ok(v) = event_target_value(&ev).parse::<i32>() { set_rating.set(v); }
                                    } />
                                </label>
                            </div>
                            <div class="row" style="gap:8px;margin-top:8px;">
                                <button class="btn" on:click=do_insert disabled=move || insert_loading.get()>
                                    {move || if insert_loading.get() {"Submitting..."} else {"Submit"}}
                                </button>
                                <Show when=move || !insert_err.get().is_empty()>
                                    {move || view!{<span class="danger">{insert_err.get()}</span>}}
                                </Show>
                            </div>
                        </div>
                        <div class="card">
                            <div style="font-weight:600;margin-bottom:8px;">"Response"</div>
                            <pre>{move || insert_resp.get()}</pre>
                        </div>
                    </div>
                }.into_view(),
                Tab::Bulk => view! {
                    <div class="card">
                        <div class="row" style="justify-content:space-between;margin-bottom:8px;">
                            <div style="font-weight:600;">"Bulk Insert Reviews"</div>
                            <div class="row">
                                <button on:click=add_bulk_row>"+ Add Row"</button>
                                <button class="btn" on:click=do_bulk disabled=move || bulk_loading.get()>
                                    {move || if bulk_loading.get() {"Submitting..."} else {"Submit Bulk"}}
                                </button>
                            </div>
                        </div>
                        <div style="overflow:auto;">
                            <table>
                                <thead><tr><th>Title</th><th>Body</th><th>Product ID</th><th>Rating</th><th>Actions</th></tr></thead>
                                <tbody>
                                    {move || {
                                        let items = bulk_items.get();
                                        items.into_iter().enumerate().map(|(i, it)| view!{
                                            <tr>
                                                <td><input prop:value=it.review_title on:input=move |ev| set_bulk_items.update(|v| v[i].review_title = event_target_value(&ev)) /></td>
                                                <td><textarea on:input=move |ev| set_bulk_items.update(|v| v[i].review_body = event_target_value(&ev))>{it.review_body}</textarea></td>
                                                <td><input prop:value=it.product_id on:input=move |ev| set_bulk_items.update(|v| v[i].product_id = event_target_value(&ev)) /></td>
                                                <td><input type="number" prop:value=it.review_rating.to_string() on:input=move |ev| if let Ok(v)=event_target_value(&ev).parse(){ set_bulk_items.update(|vct| vct[i].review_rating = v); } /></td>
                                                <td><button on:click=move |_| remove_bulk_row(i)>"Remove"</button></td>
                                            </tr>
                                        }).collect::<Vec<_>>()
                                    }}
                                </tbody>
                            </table>
                        </div>
                        <Show when=move || !bulk_err.get().is_empty()>
                            {move || view!{<div class="danger" style="margin-top:8px;">{bulk_err.get()}</div>}}
                        </Show>
                        <div class="card" style="margin-top:16px;">
                            <div style="font-weight:600;margin-bottom:8px;">"Response"</div>
                            <pre>{move || bulk_resp.get()}</pre>
                        </div>
                    </div>
                }.into_view(),
                Tab::Search => view! {
                    <div class="grid cols-2">
                        <div class="card">
                            <div style="font-weight:600;margin-bottom:8px;">"Search Reviews"</div>
                            <label>
                                <span>"Query"</span>
                                <input prop:value=move || query.get() on:input=move |ev| set_query.set(event_target_value(&ev)) />
                            </label>
                            <label style="width:160px">
                                <span>"Top K"</span>
                                <input type="number" prop:value=move || top_k.get().to_string() on:input=move |ev| if let Ok(v)=event_target_value(&ev).parse(){ set_top_k.set(v) } />
                            </label>
                            <div style="margin-top:8px;">
                                <button class="btn" on:click=do_search disabled=move || search_loading.get()>
                                    {move || if search_loading.get() {"Searching..."} else {"Search"}}
                                </button>
                                <Show when=move || !search_err.get().is_empty()>
                                    {move || view!{<span class="danger" style="margin-left:8px;">{search_err.get()}</span>}}
                                </Show>
                            </div>
                        </div>
                        <div class="card">
                            <div style="font-weight:600;margin-bottom:8px;">"Response"</div>
                            <pre>{move || search_resp.get()}</pre>
                        </div>
                    </div>
                }.into_view(),
            }}

            <div class="row" style="margin-top:18px;color:var(--muted);font-size:12px;">
                "Built for POST /reviews, /reviews/bulk, /search"
            </div>
        </div>
    }
}
