using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Shapes;

namespace ConvertZZ.Moudle
{
    /// <summary>
    /// Window_MessageBoxEx.xaml 的互動邏輯
    /// </summary>
    public partial class Window_MessageBoxEx : Window, INotifyPropertyChanged,IDisposable
    {
        MessageBoxExResult Resoult= MessageBoxExResult.NONE;
        public Window_MessageBoxEx(string Text,string Caption,string button1_Text, string button2_Text, string button3_Text)
        {
            TextBlock_Text = Text;            
            ButtonText1 = button1_Text;
            ButtonText2 = button2_Text;
            ButtonText3 = button3_Text;
            DataContext = this;
            InitializeComponent();
            this.Title = Caption;
        }
        public Window_MessageBoxEx(string Text, string Caption, string button1_Text, string button2_Text, string button3_Text,string CheckBox_Text)
        {
            TextBlock_Text = Text;
            ButtonText1 = button1_Text;
            ButtonText2 = button2_Text;
            ButtonText3 = button3_Text;
            this.CheckBox_Text = CheckBox_Text;
            DataContext = this;
            InitializeComponent();
            this.Title = Caption;
        }
        public enum MessageBoxExResult
        {
            NONE = 0,
            /// <summary>
            /// 第一個按鈕
            /// </summary>
            A = 1,
            /// <summary>
            /// 第二個按鈕
            /// </summary>
            B = 3,
            /// <summary>
            /// 第三個按鈕
            /// </summary>
            C = 4,
            /// <summary>
            /// 第一個按鈕+打勾
            /// </summary>
            AO = 2,
            /// <summary>
            /// 第二個按鈕+打勾
            /// </summary>
            BO = 6,
            /// <summary>
            /// 第三個按鈕+打勾
            /// </summary>
            CO = 8
        }
       




        private string _TextBlock_Text, _ButtonText1, _ButtonText2, _ButtonText3, _CheckBox_Text;
        private Visibility _CheckBox_Visibility = Visibility.Collapsed;
        private bool _CheckBox_IsChecked = false;
        public string TextBlock_Text { get => _TextBlock_Text; set { _TextBlock_Text = value; OnPropertyChanged("TextBlock_Text"); } }
        public string ButtonText1 { get => _ButtonText1; set { _ButtonText1 = value; OnPropertyChanged("ButtonText1"); } }
        public string ButtonText2 { get => _ButtonText2; set { _ButtonText2 = value; OnPropertyChanged("ButtonText2"); } }
        public string CheckBox_Text { get => _CheckBox_Text; set { _CheckBox_Text = value; CheckBox_Visibility = (String.IsNullOrEmpty(value) ? Visibility.Collapsed : Visibility.Visible); OnPropertyChanged("CheckBox_Text"); } }
        public Visibility CheckBox_Visibility { get => _CheckBox_Visibility; set { _CheckBox_Visibility = value; OnPropertyChanged("CheckBox_Visibility"); } }
        public bool CheckBox_IsChecked { get => _CheckBox_IsChecked; set { _CheckBox_IsChecked = value; OnPropertyChanged("CheckBox_IsChecked"); } }
    
        private void Button_Click(object sender, RoutedEventArgs e)
        {
            Resoult = (MessageBoxExResult)(1 * (CheckBox_IsChecked ? 2 : 1));
            this.Close();
        }

        private void Button_Click_1(object sender, RoutedEventArgs e)
        {
            Resoult = (MessageBoxExResult)(3 * (CheckBox_IsChecked ? 2 : 1));
            this.Close();
        }

        private void Button_Click_2(object sender, RoutedEventArgs e)
        {
            Resoult = (MessageBoxExResult)(4 * (CheckBox_IsChecked ? 2 : 1));
            this.Close();
        }

        public string ButtonText3 { get => _ButtonText3; set { _ButtonText3 = value; OnPropertyChanged("ButtonText3"); } }

        public event PropertyChangedEventHandler PropertyChanged;

      

        protected void OnPropertyChanged(string name) => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));

        #region IDisposable Support
        private bool disposedValue = false; // 偵測多餘的呼叫

        protected virtual void Dispose(bool disposing)
        {
            if (!disposedValue)
            {
                if (disposing)
                {
                    // TODO: 處置 Managed 狀態 (Managed 物件)。
                }

                // TODO: 釋放 Unmanaged 資源 (Unmanaged 物件) 並覆寫下方的完成項。
                // TODO: 將大型欄位設為 null。

                disposedValue = true;
            }
        }

        // TODO: 僅當上方的 Dispose(bool disposing) 具有會釋放 Unmanaged 資源的程式碼時，才覆寫完成項。
        // ~Window_MessageBoxEx() {
        //   // 請勿變更這個程式碼。請將清除程式碼放入上方的 Dispose(bool disposing) 中。
        //   Dispose(false);
        // }

        // 加入這個程式碼的目的在正確實作可處置的模式。
        void IDisposable.Dispose()
        {
            // 請勿變更這個程式碼。請將清除程式碼放入上方的 Dispose(bool disposing) 中。
            Dispose(true);
            // TODO: 如果上方的完成項已被覆寫，即取消下行的註解狀態。
            // GC.SuppressFinalize(this);
        }
        #endregion

    }
    public partial class Window_MessageBoxEx
    {
        public static MessageBoxExResult Show(string Text, string Caption, string button1_Text, string button2_Text, string button3_Text)
        {
            // using construct ensures the resources are freed when form is closed
            using (var form = new Window_MessageBoxEx(Text, Caption, button1_Text, button2_Text, button3_Text))
            {
                form.ShowDialog();
                return form.Resoult;
            }
        }
        public static MessageBoxExResult Show(string Text, string Caption, string button1_Text, string button2_Text, string button3_Text, string CheckBox_Text)
        {
            // using construct ensures the resources are freed when form is closed
            using (var form = new Window_MessageBoxEx(Text, Caption, button1_Text, button2_Text, button3_Text, CheckBox_Text))
            {
                form.ShowDialog();
                return form.Resoult;
            }
        }
    }
}
