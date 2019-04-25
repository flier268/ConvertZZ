using System;
using System.Collections.Specialized;
using System.Diagnostics;
using System.Net;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Input;

namespace ConvertZZ
{
    /// <summary>
    /// Window_Report.xaml 的互動邏輯
    /// </summary>
    public partial class Window_Report : Window
    {
        public Window_Report()
        {
            InitializeComponent();
        }

        private void Window_MouseDown(object sender, MouseButtonEventArgs e)
        {
            if (e.LeftButton == MouseButtonState.Pressed)
            {
                this.DragMove();
            }
        }

        private void Button_Close_Click(object sender, RoutedEventArgs e)
        {
            Close();
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            if (TextBox_Name.Text == "" || TextBox_Title.Text == "")
            {
                MessageBox.Show(this, "姓名與標題為必填");
                return;
            }
            Button_Send.Content = "發送中...";
            Button_Send.IsEnabled = false;
            SendGoogleFormAsync(TextBox_Name.Text, TextBox_Email.Text, TextBox_Title.Text, Textbox_Content.Text);
            Button_Send.Content = "送出";
            Button_Send.IsEnabled = true;
        }
        private async void SendGoogleFormAsync(string Name, string Email, string Title, string Content)
        {
            await Task.Run(() =>
             {
                 try
                 {
                     WebClient client = new WebClient();
                     var keyValue = new NameValueCollection
                     {
                         { "entry.1633774934", Name },
                         { "entry.1516933929", Email },
                         { "entry.586000024", Title },
                         { "entry.1890682106", Content }
                     };
                     Uri uri = new Uri("https://docs.google.com/forms/d/e/1FAIpQLSfCOOFaY8vx-isqg6y3J2QXhF88VyVnpW2Cdw-opZZHPMECbg/formResponse");
                     byte[] response = client.UploadValues(uri, "POST", keyValue);
                     string result = Encoding.UTF8.GetString(response);
                     MessageBox.Show("感謝您的回報");
                     Action methodDelegate = delegate ()
                     {
                         Close();
                     };
                     this.Dispatcher.BeginInvoke(methodDelegate);
                 }
                 catch
                 {
                     MessageBox.Show("回報單傳送失敗，請確認網路連線狀態");
                 }
             });
        }

        private void Text_Github_Click(object sender, RoutedEventArgs e)
        {
            Process.Start("https://github.com/flier268/ConvertZZ/issues");
        }
    }
}
