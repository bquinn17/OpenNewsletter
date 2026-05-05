I'd like to create a new app called open newsletter.   
This app will have several key features at a high level. These include the following the ability to have a monthly newsletter where a group of users (friends) get together to submit questions, collect answers from all of the friends to those questions, and then show the answers in a summary newsletter. These questions will be Long form text along with attached images and gifs. The app also needs a screen that will show the list of past newsletters and upcoming newsletters. 

I want to be able to host this app in AWS with a minimum cost possible. I would like to get feedback on the best way to do this, but the way I'm thinking about it is storing all images and media in Amazon S3. Using a cheap database to store the answers to the questions, user data, and any other structured data. and using some kind of serverless infrastructure to host the app. I would also likely need some kind of sqs and SNS service to be able to notify users when it is time to fill out the newsletter and when the newsletter has been published. 

I want the entire front end of the app to be a separate static front end. This could be hosted in GitHub pages for free.

The front end app should work as a progressive web app. It should be able to be added to a mobile user's home screen. They should also be able to enable push notifications for their mobile device. The whole front end should be mobile first and responsive. When opening the front end on a desktop, the UI screens should mostly just use the middle third of the page.

For the push notifications, the schedule should be easily configurable. For starters, when a newsletter is open for responses, users should get notifications 4 days before responses are due, 48 hours, and 24 hours. When a new newsletter Is open for question ideas to be submitted. Users should also get a notification

I would like the app to be self-contained in a single repo that also defines all the infrastructure. Perhaps this could use the Amazon SDK. 

I want this app to be designed to be “multi-tenant”, such that there are different groups of friends with newsletters of their own. A single user may potentially be part of multiple newsletters.

I will list off some other features that are important for the app. 

* As you're filling out a response to the newsletter, the progress of your form should be saved automatically to the back end. As a draft. You will be able to publish your response when it is completed. Users should be able to edit their responses up until the deadline when the newsletter is published.  
* The app should be able to support responses that are polls, were there a number of options for answers and you must select one. The newsletter will display the results of the poll.   
* I want users to be able to sign up using oauth with a Google account Facebook account, or apple account.  
* Users will only be able to sign up if they have a special invite code. I don't want a user to be able to hit any of the back end API unless they have authenticated with the static front end. This should mean that the serverless backend will only be triggered by authenticated users and can mostly stay idle. The GitHub page is front end will serve requests from non-authenticated users.  
* Within the published newsletter, users should be able to respond in comments to each of the answers that a user has added in their post. Should also be able to react with emojis to the answers.   
* The attached images in users responses should render nicely along with the text in their response.   
* Users should be able to suggest questions for the upcoming newsletter at any time by visiting the page that shows each past month's newsletter along with the upcoming newsletter and then clicking on the upcoming newsletter.   
* I also want to think about using a front-end cache like varnish to store a cache of images on the device, in order to minimize the cost of accessing S3.  
* Eventually, I'd like to create a job that will archive old newsletters, so that they will no longer be sitting in expensive services like S3 and hot databases.