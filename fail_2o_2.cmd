cargo nextest run --no-fail-fast --test-threads=1 --\
 rig_test::orchestrator_basic_tests test_rag_query_immediate_execution\
 rig_test::orchestrator_missing_context_tests test_describe_report_missing_both_ids\
 rig_test::orchestrator_workflow_tests test_compare_workflow_step2_analyze_reports\
 rig_test::orchestrator_workflow_tests test_describe_latest_workflow