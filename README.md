# Rust, Lambda, OpenTelemetry, Connected with Datadog

This project demonstrates how to implement distributed tracing in AWS Lambda functions written in Rust using OpenTelemetry and Datadog. It shows how to propagate trace context across multiple services and through an SQS queue.

## Architecture

The solution consists of three Rust Lambda functions:

1. **Post Function**: Handles HTTP POST requests, calls the Read Function, and sends messages to an SQS queue
2. **Read Function**: Handles HTTP GET requests and returns context data
3. **Handle Change Function**: Processes messages from the SQS queue

The workflow demonstrates trace context propagation:
- The Post Function initiates a trace and calls the Read Function
- The Read Function continues the trace and returns data
- The Post Function sends the data to an SQS queue with the trace context
- The Handle Change Function processes the message and continues the trace

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [AWS CLI](https://aws.amazon.com/cli/)
- [AWS CDK](https://docs.aws.amazon.com/cdk/latest/guide/getting_started.html)
- [cargo-lambda](https://www.cargo-lambda.info/guide/getting-started.html)
- [Node.js](https://nodejs.org/) (for CDK)
- Datadog account with API key

## Environment Variables

Set the following environment variables:

```bash
export DD_API_KEY=your_datadog_api_key
export DD_SITE=datadoghq.com  # or your specific Datadog site
```

## Deployment

1. Clone the repository
2. Install dependencies:
   ```bash
   cd infra
   npm install
   ```
3. Build the Rust Lambda functions:
   ```bash
   cd lambdas
   cargo build --release
   ```
4. Deploy with CDK:
   ```bash
   cd infra
   cdk deploy
   ```

## Usage

After deployment, you'll get an API Gateway URL. You can test the solution with:

```bash
# Trigger the workflow with a POST request
curl -X POST https://your-api-id.execute-api.region.amazonaws.com/demo/

# Directly call the Read Function
curl https://your-api-id.execute-api.region.amazonaws.com/demo/
```

## Monitoring

Check your Datadog APM dashboard to see the distributed traces across the Lambda functions and SQS queue.

## Learn More

For more detailed information and insights about this solution, visit:
[OpenTelemetry with Rust Lambda and Datadog](https://binaryheap.com/opentelemet-rust-lambda-datadog/)
