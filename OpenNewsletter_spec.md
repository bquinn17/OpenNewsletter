# **OpenNewsletter Specification**

## **The Final, Low-Complexity Blueprint**

1. **Frontend:** Hosted on GitHub Pages (Static HTML/JS/CSS). Uses Progressive Web App (PWA) Service Workers and Cache Storage API to cache assets on the device and avoid Varnish/server costs.  
2. **Auth:** Amazon Cognito. Handles OAuth (Google, Apple, Facebook) and issues JWTs.  
3. **API & Security:** Amazon API Gateway \+ AWS Lambda. API Gateway validates Cognito tokens before invoking Lambda, ensuring the backend stays idle and secure against unauthenticated traffic.  
4. **Database:** Amazon DynamoDB. A Single-Table NoSQL design storing Users, Groups, Invites, Questions, Answers, Comments, and Reactions. Sort keys naturally handle multi-tenancy isolation and "last-write-wins" idempotency for reactions/polls.  
5. **Storage:** Amazon S3. Frontend requests a Pre-Signed URL from Lambda, then uploads images directly to S3 (capped at 15MB, restricted MIME types).  
6. **Notifications:** Amazon EventBridge triggers Lambda functions on a schedule (e.g., 4 days, 48 hours, 24 hours before deadlines), which publish to Amazon SNS to handle PWA Push Notifications.  
7. **Infrastructure as Code:** AWS Cloud Development Kit (CDK). The frontend code and the AWS infrastructure definitions live in a single Git repository.

## **1\. Project Overview**

The project is a new application called Open Newsletter. It facilitates a monthly newsletter process where groups of friends can submit questions, collect long-form text answers (with attached images and GIFs) from the group, and view the compiled results in a summary format. The app must be designed for multi-tenancy, allowing different groups of friends to have isolated newsletters, with individual users having the ability to participate in multiple groups.

## **2\. Hosting & Infrastructure**

To minimize costs, the application will be hosted on AWS using a fully serverless architecture.

* **Frontend:** A separate static frontend hosted for free on GitHub Pages.  
* **Compute:** AWS Lambda combined with Amazon API Gateway. The backend API will only be triggered by authenticated users, remaining mostly idle otherwise.  
* **Database:** A cheap NoSQL database (Amazon DynamoDB) utilizing a Single-Table Design to store user data, structured data, questions, and answers.  
* **Media Storage:** Amazon S3 will store all images and media. File uploads will be constrained by size and type via pre-signed URLs to prevent abuse.  
* **Codebase:** The entire application and its infrastructure will be self-contained in a single repository using Infrastructure as Code (AWS CDK).

## **3\. Frontend & User Experience**

* **Platform:** The frontend will be a Progressive Web App (PWA) that can be added to a mobile device's home screen.  
* **Design:** It will be a mobile-first, responsive design; on desktop environments, the UI will be constrained to the middle third of the screen. Attached images will render nicely alongside text responses.  
* **Caching:** To minimize S3 access costs, the PWA will utilize native Service Workers and the browser Cache Storage API to cache images and assets locally on the device.  
* **Navigation:** A dedicated screen will display a list of past and upcoming newsletters. Users can suggest questions for upcoming newsletters at any time by navigating to this list and clicking on an upcoming edition.

## **4\. Authentication & Access**

* **Login:** Users will authenticate via OAuth using Google, Facebook, or Apple accounts.  
* **Group Management:** Registration is strictly gated by special invite codes. These codes are tied to specific groups. Upon redemption, the user is linked to that group. Existing users can use new invite codes to join additional groups.  
* **Visibility:** Users will only see data for the groups they are explicitly a part of. Admins will generate these invite codes manually via the AWS Console to reduce initial app complexity.

## **5\. Core Newsletter Features**

* **Drafts & Editing:** As users fill out responses, their progress will automatically save to the backend as a draft. Users can edit these responses until the final publication deadline, at which point they are finalized.  
* **Polls:** The application will support poll-based questions with predefined options, and the final newsletter will display the compiled poll results. Users can only vote once, with the last vote submitted taking precedence.  
* **Engagement:** Within a published newsletter, users can reply to specific answers via nested comments and react to answers using emojis. Emoji reactions will utilize last-write-wins logic per user.

## **6\. Notifications**

The app will utilize scheduled jobs (Amazon EventBridge) and a notification service (Amazon SNS) to manage user alerts. Users must be able to enable push notifications on their devices.

* **Schedule:** The notification schedule must be easily configurable.  
* **Triggers:** Notifications will be sent when a new newsletter opens for question submissions. Additionally, reminders will be sent 4 days, 48 hours, and 24 hours before response deadlines.

## **7\. Future Considerations**

* **Data Archival:** A background job will eventually be created to archive older newsletters, moving them out of hot database storage and standard S3 into cheaper cold storage to minimize long-term costs.