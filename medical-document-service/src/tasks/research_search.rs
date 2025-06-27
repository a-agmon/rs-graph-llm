use super::utils::get_llm_agent;
use crate::models::{MedicalDocument, ResearchArticle};
use async_trait::async_trait;
use chrono::Datelike;
use graph_flow::{Context, GraphError, NextAction, Result, Task, TaskResult};
use reqwest;
use rig::completion::Prompt;
use serde_json::Value;
use tracing::{error, info, warn};

pub struct ResearchSearchTask;

#[async_trait]
impl Task for ResearchSearchTask {
    async fn run(&self, context: Context) -> Result<TaskResult> {
        info!("Starting medical research search");

        let document: MedicalDocument = context
            .get("document")
            .await
            .ok_or_else(|| GraphError::ContextError("Document not found in context".to_string()))?;

        let integrated_summary = document
            .integrated_summary
            .as_ref()
            .ok_or_else(|| GraphError::ContextError("Integrated summary not found".to_string()))?;

        // Generate search queries from the integrated summary
        let search_queries = match generate_search_queries(integrated_summary).await {
            Ok(queries) => queries,
            Err(e) => {
                error!("Failed to generate search queries: {}", e);
                return Err(GraphError::TaskExecutionFailed(format!(
                    "Search query generation failed: {}",
                    e
                )));
            }
        };

        info!("Generated search queries: {:?}", search_queries);

        // Search PubMed for relevant articles
        let research_articles = match search_pubmed(&search_queries).await {
            Ok(articles) => articles,
            Err(e) => {
                error!("PubMed search failed: {}", e);
                // Continue with empty research rather than failing
                warn!("Continuing without research articles due to search failure");
                Vec::new()
            }
        };

        info!("Found {} research articles", research_articles.len());

        // Generate research summary from found articles
        let research_summary = if research_articles.is_empty() {
            "No recent relevant medical literature found for this case.".to_string()
        } else {
            match generate_research_summary(integrated_summary, &research_articles).await {
                Ok(summary) => summary,
                Err(e) => {
                    error!("Failed to generate research summary: {}", e);
                    return Err(GraphError::TaskExecutionFailed(format!(
                        "Research summary generation failed: {}",
                        e
                    )));
                }
            }
        };

        // Update document with research data
        let mut updated_document = document;
        updated_document.research_keywords = Some(search_queries);
        //updated_document.research_articles = Some(research_articles);
        updated_document.research_summary = Some(research_summary.clone());
        context.set("document", updated_document).await;

        info!("Medical research search completed");

        Ok(TaskResult::new_with_status(
            Some(research_summary),
            NextAction::End,
            Some("Medical document analysis completed successfully".to_string()),
        ))
    }
}

async fn generate_search_queries(summary: &str) -> anyhow::Result<Vec<String>> {
    let prompt = format!(
        r#"You are a medical research assistant specializing in PubMed literature search.
        
        Based on this medical summary, generate 2 PubMed search queries that would help find relevant recent research articles.
        
        IMPORTANT SEARCH QUERY GUIDELINES:
        - Use quotation marks around multi-word medical terms for exact phrases
        - Use OR between related terms to broaden search results
        - Use AND only when combining different concepts
        - Avoid overly restrictive queries that combine too many terms with AND
        - Focus on primary medical conditions and key findings
        
        EXAMPLES of good search queries:
        - "Normal Pressure Hydrocephalus" OR "NPH" OR "gait disturbance"
        - "ventricular enlargement" AND ("cognitive impairment" OR "dementia")
        - "ischemic heart disease" OR "coronary artery disease"
        
        Generate exactly 2 search queries:
        1. Primary condition focused (more specific)
        2. Broader symptom/finding focused (more general)
        
        Return only the queries as a JSON array of strings, nothing else.
        
        Medical Summary:
        {}
        
        Search Queries (JSON array only):"#,
        summary
    );

    let agent =
        get_llm_agent("You are a medical research assistant specializing in literature search.")?;
    let response = agent.prompt(&prompt).await?;

    info!("LLM response for search queries: {}", response);

    // Try to extract JSON array from the response
    let queries = if let Some(json_start) = response.find('[') {
        if let Some(json_end) = response.rfind(']') {
            let json_str = &response[json_start..=json_end];
            serde_json::from_str::<Vec<String>>(json_str)
                .map_err(|e| anyhow::anyhow!("Failed to parse extracted JSON: {}", e))?
        } else {
            return Err(anyhow::anyhow!("No closing bracket found in response"));
        }
    } else {
        // Fallback: try to parse the entire response as JSON
        serde_json::from_str::<Vec<String>>(&response)
            .map_err(|e| anyhow::anyhow!("Failed to parse response as JSON: {}", e))?
    };

    Ok(queries)
}

async fn search_pubmed(search_queries: &[String]) -> anyhow::Result<Vec<ResearchArticle>> {
    let client = reqwest::Client::new();
    let base_url = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils";
    let current_year = chrono::Utc::now().year();
    let years_back = 3; // Search last 3 years

    // Try each search query until we find results
    for (index, search_term) in search_queries.iter().enumerate() {
        info!(
            "Trying search query {} of {}: {} (years: {}-{})",
            index + 1,
            search_queries.len(),
            search_term,
            current_year - years_back,
            current_year
        );

        // First, search for PMIDs
        let search_url = format!(
            "{}/esearch.fcgi?db=pubmed&term={}&datetype=pdat&mindate={}&maxdate={}&retmax=10&retmode=json",
            base_url,
            urlencoding::encode(search_term),
            current_year - years_back,
            current_year
        );

        let search_response = client
            .get(&search_url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("PubMed search request failed: {}", e))?;

        let search_data: Value = search_response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse search response: {}", e))?;

        let pmids = search_data["esearchresult"]["idlist"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("No PMIDs found in search results"))?;

        if !pmids.is_empty() {
            info!(
                "Search query {} found {} articles, fetching details",
                index + 1,
                pmids.len()
            );

            // Fetch article details
            let pmid_list = pmids
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(",");

            let fetch_url = format!(
                "{}/efetch.fcgi?db=pubmed&id={}&retmode=xml",
                base_url, pmid_list
            );

            let fetch_response = client
                .get(&fetch_url)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("PubMed fetch request failed: {}", e))?;

            let xml_content = fetch_response
                .text()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get fetch response text: {}", e))?;

            // For simplicity, we'll parse key information from XML manually
            // In a production system, you'd use a proper XML parser
            let articles = parse_pubmed_xml(&xml_content)?;
            return Ok(articles);
        } else {
            info!("Search query {} found no articles", index + 1);
        }
    }

    warn!(
        "No articles found with any search query: {:?}",
        search_queries
    );
    Ok(Vec::new())
}

fn parse_pubmed_xml(xml: &str) -> anyhow::Result<Vec<ResearchArticle>> {
    // This is a simplified XML parsing - in production use a proper XML parser
    let mut articles = Vec::new();

    // Split by article entries (very basic parsing)
    let article_sections: Vec<&str> = xml.split("<PubmedArticle>").collect();

    for section in article_sections.iter().skip(1) {
        // Skip first empty split
        if let Some(pmid) = extract_xml_value(section, "<PMID") {
            let title = extract_xml_value(section, "<ArticleTitle>").unwrap_or_default();
            let abstract_text = extract_xml_value(section, "<AbstractText>").unwrap_or_default();
            let journal = extract_xml_value(section, "<Title>").unwrap_or_default();

            articles.push(ResearchArticle {
                pmid,
                title,
                abstract_text,
                authors: None,
                journal: Some(journal),
                publication_date: None,
            });
        }
    }

    Ok(articles)
}

fn extract_xml_value(xml: &str, tag: &str) -> Option<String> {
    let start_tag = if tag.contains('<') {
        tag
    } else {
        &format!("<{}>", tag)
    };
    let end_tag = if tag.contains('<') {
        tag.replace('<', "</").replace(' ', ">")
    } else {
        format!("</{}>", tag)
    };

    if let Some(start) = xml.find(start_tag) {
        let content_start = xml[start..].find('>')? + start + 1;
        if let Some(end) = xml[content_start..].find(&end_tag) {
            let content = &xml[content_start..content_start + end];
            return Some(content.trim().to_string());
        }
    }
    None
}

async fn generate_research_summary(
    summary: &str,
    articles: &[ResearchArticle],
) -> anyhow::Result<String> {
    let articles_text = articles
        .iter()
        .map(|article| {
            format!(
                "Title: {}\nAbstract: {}\n\n",
                article.title, article.abstract_text
            )
        })
        .collect::<Vec<_>>()
        .join("\n---\n");

    let prompt = format!(
         "You are a medical research analyst. 
          Review the patient's medical summary, and a number of recent research articles that might be relevant to the patient's condition or to the diagnosis.
          Examine whether the research articles provide additional information or second opinion on the treatment options and the best course of action.
          Provide reference to the research articles you mention in the summary.
          Return only your summary and suggestions as a string, nothing else. 
          

        Patient Summary:
        {}

        Recent Research Articles:
        {}

        Provide a structured research analysis:",
        summary,
        articles_text
    );

    let agent = get_llm_agent(
        "You are a medical research analyst specializing in clinical literature review.",
    )?;
    let response = agent.prompt(&prompt).await?;
    Ok(response)
}
