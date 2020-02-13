using System;
using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;

namespace ConvertZZ.Moudle
{
    /// <summary>
    /// Window_MessageBoxEx.xaml 的互動邏輯
    /// </summary>
    public partial class Window_MessageBoxEx : Window, INotifyPropertyChanged, IDisposable
    {
        MessageBoxExResult Resoult = MessageBoxExResult.NONE;

        public Window_MessageBoxEx(string Text, string Caption, string button1_Text, string button2_Text, string button3_Text, string CheckBox_Text)
        {
            TextBlock_Text = Text;
            ButtonText1 = button1_Text;
            Button1_Width = new GridLength(1.0, GridUnitType.Star);
            Button2_Width = new GridLength(1.0, GridUnitType.Star);
            if (button2_Text == null)
            {
                Button2_Visibility = Visibility.Collapsed;
                Button2_Width = new GridLength(1.0, GridUnitType.Auto);
            }
            else
            {
                ButtonText2 = button2_Text;
            }
            if (button3_Text == null)
            {
                Button3_Visibility = Visibility.Collapsed;
                Button3_Width = new GridLength(1.0, GridUnitType.Auto);
            }
            else
            {
                ButtonText3 = button3_Text;
            }
            if (CheckBox_Text == null)
            {
                CheckBox_Visibility = Visibility.Collapsed;
            }
            else
            {
                this.CheckBox_Text = CheckBox_Text;
            }
            base.DataContext = this;
            InitializeComponent();
            Title = Caption;
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
        public enum Type
        {
            WithoutCheckBox,
            WithCheckBox
        }




        private string _ButtonText3, _CheckBox_Text;
        private bool _CheckBox_IsChecked = false;
        public string TextBlock_Text { get; set; }
        public string ButtonText1 { get; set; }
        public string ButtonText2 { get; set; }
        public string CheckBox_Text { get => _CheckBox_Text; set { _CheckBox_Text = value; CheckBox_Visibility = (String.IsNullOrEmpty(value) ? Visibility.Collapsed : Visibility.Visible); } }
        public Visibility CheckBox_Visibility { get; set; } = Visibility.Collapsed;
        public bool CheckBox_IsChecked { get => _CheckBox_IsChecked; set { _CheckBox_IsChecked = value; } }

        private void Button_Click_1(object sender, RoutedEventArgs e)
        {
            Resoult = (MessageBoxExResult)(1 * (CheckBox_IsChecked ? 2 : 1));
            this.Close();
        }

        private void Button_Click_2(object sender, RoutedEventArgs e)
        {
            Resoult = (MessageBoxExResult)(3 * (CheckBox_IsChecked ? 2 : 1));
            this.Close();
        }

        private void Button_Click_3(object sender, RoutedEventArgs e)
        {
            Resoult = (MessageBoxExResult)(4 * (CheckBox_IsChecked ? 2 : 1));
            this.Close();
            ColumnDefinition columnDefinition = new ColumnDefinition();
            columnDefinition.Width = new GridLength(1, GridUnitType.Star);
        }

        public string ButtonText3 { get => _ButtonText3; set { _ButtonText3 = value; } }
        private GridLength _Button1_Width = new GridLength();
        private GridLength _Button2_Width = new GridLength();
        private GridLength _Button3_Width = new GridLength();
        private Visibility _Button3_Visibility = Visibility.Visible;
        public Visibility Button1_Visibility { get; set; } = Visibility.Visible;
        public Visibility Button2_Visibility { get; set; } = Visibility.Visible;
        public Visibility Button3_Visibility { get => _Button3_Visibility; set { _Button3_Visibility = value; } }
        public GridLength Button1_Width { get => _Button1_Width; set { _Button1_Width = value; } }
        public GridLength Button2_Width { get => _Button2_Width; set { _Button2_Width = value; } }
        public GridLength Button3_Width { get => _Button3_Width; set { _Button3_Width = value; } }

        public event PropertyChangedEventHandler PropertyChanged;

        private void ThisWindows_MouseDown(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            if (e.LeftButton == System.Windows.Input.MouseButtonState.Pressed)
                this.DragMove();
        }




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
        public static MessageBoxExResult ShowDialog(string Text, string Caption, string button1_Text, string button2_Text, string button3_Text, string CheckBox_Text)
        {
            // using construct ensures the resources are freed when form is closed
            using (Window_MessageBoxEx window_MessageBoxEx = new Window_MessageBoxEx(Text, Caption, button1_Text, button2_Text, button3_Text, CheckBox_Text))
            {
                window_MessageBoxEx.ShowDialog();
                return window_MessageBoxEx.Resoult;
            }
        }
        public static MessageBoxExResult ShowDialog(string Text, string Caption, string button1_Text, string button2_Text, string button3_Text)
        {
            return ShowDialog(Text, Caption, button1_Text, button2_Text, button3_Text, null);
        }
        public static MessageBoxExResult ShowDialog(string Text, string Caption, string button1_Text, string button2_Text)
        {
            return ShowDialog(Text, Caption, button1_Text, button2_Text, null);
        }
        public static MessageBoxExResult ShowDialog(string Text, string Caption, string button1_Text)
        {
            return ShowDialog(Text, Caption, button1_Text, null);
        }
    }
}
