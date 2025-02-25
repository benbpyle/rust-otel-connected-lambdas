import * as cdk from 'aws-cdk-lib';
import { LambdaIntegration, RestApi } from 'aws-cdk-lib/aws-apigateway';
import { Architecture, LayerVersion } from 'aws-cdk-lib/aws-lambda';
import { SqsEventSource } from 'aws-cdk-lib/aws-lambda-event-sources';
import { Queue } from 'aws-cdk-lib/aws-sqs';
import { RustFunction } from 'cargo-lambda-cdk';
import { Construct } from 'constructs';
import path = require('path');
// import * as sqs from 'aws-cdk-lib/aws-sqs';

export class DatadogQueueConnectedStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    let api = new RestApi(this, "RestApi", {
      description: 'Sample API',
      restApiName: 'Sample API',
      disableExecuteApiEndpoint: false,
      deployOptions: {
        stageName: 'demo',
      },
    });

    const layer = LayerVersion.fromLayerVersionArn(
      this,
      'DatadogExtension',
      'arn:aws:lambda:us-east-1:464622532012:layer:Datadog-Extension-ARM:68'
    )

    const queue = new Queue(this, 'PostQueue', {
      queueName: 'sample-post-queue'
    });

    const postFunction = new RustFunction(this, 'PostFunction', {
      architecture: Architecture.ARM_64,
      functionName: "sample-post-function",
      manifestPath: path.join(__dirname, `../../lambdas/post-function`),
      memorySize: 256,
      environment: {
        RUST_LOG: 'info',
        FUNCTION_NAME: "post-function",
        DD_API_KEY: process.env.DD_API_KEY!,
        DD_SITE: process.env.DD_SITE!,
        AGENT_ADDRESS: '127.0.0.1',
        QUEUE_URL: queue.queueUrl,
        API_URL: api.deploymentStage.urlForPath()
      },
      layers: [layer]
    });

    const readFunction = new RustFunction(this, 'ReadFunction', {
      architecture: Architecture.ARM_64,
      functionName: "sample-read-function",
      manifestPath: path.join(__dirname, `../../lambdas/read-function`),
      memorySize: 256,
      environment: {
        RUST_LOG: 'info',
        FUNCTION_NAME: "read-function",
        DD_API_KEY: process.env.DD_API_KEY!,
        DD_SITE: process.env.DD_SITE!,
        AGENT_ADDRESS: '127.0.0.1'
      },
      layers: [layer]
    });

    const changeFunction = new RustFunction(this, 'ChangeFunction', {
      architecture: Architecture.ARM_64,
      functionName: "sample-handle-change-function",
      manifestPath: path.join(__dirname, `../../lambdas/handle-change-function`),
      memorySize: 256,
      environment: {
        RUST_LOG: 'info',
        FUNCTION_NAME: "handle-change-function",
        DD_API_KEY: process.env.DD_API_KEY!,
        DD_SITE: process.env.DD_SITE!,
        AGENT_ADDRESS: '127.0.0.1'
      },
      layers: [layer]
    });

    api.root.addMethod("POST", new LambdaIntegration(postFunction))
    api.root.addMethod("GET", new LambdaIntegration(readFunction))


    queue.grantSendMessages(postFunction);
    queue.grantConsumeMessages(changeFunction);
    changeFunction.addEventSource(new SqsEventSource(queue, {
      batchSize: 10,
    }))
  }
}
